// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{
    messaging::{MessagingError, SessionKey},
    presence::Presence,
};
use async_trait::async_trait;
use rgg_domain::UserId;
use serde::{Deserialize, Serialize};

pub const PAGE_SIZE: usize = 20;
pub const MAX_PROFILES: i64 = 10000;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gender {
    Female,
    Male,
}
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub first_name: String,
    pub last_name: String,
    pub nickname: String,
    pub city: String,
    pub family_name: String,
    pub family_city: String,
    pub gender: Option<Gender>,
    pub birth_year: Option<u16>,
}
impl Profile {
    pub fn validate(&self) -> Result<(), MessagingError> {
        for text in [
            &self.first_name,
            &self.last_name,
            &self.nickname,
            &self.city,
            &self.family_name,
            &self.family_city,
        ] {
            validate_text(text)?;
        }
        if self.birth_year.is_some_and(|v| !(1900..=2106).contains(&v)) {
            return Err(MessagingError::Invalid);
        }
        Ok(())
    }
    pub fn is_empty(&self) -> bool {
        [
            &self.first_name,
            &self.last_name,
            &self.nickname,
            &self.city,
            &self.family_name,
            &self.family_city,
        ]
        .iter()
        .all(|v| v.trim().is_empty())
            && self.birth_year.is_none()
    }
}
pub fn validate_text(text: &str) -> Result<(), MessagingError> {
    if text.len() > 180 || text.chars().count() > 60 || text.chars().any(char::is_control) {
        Err(MessagingError::Invalid)
    } else {
        Ok(())
    }
}
/// Explicit Polish folding; substring matching treats SQL wildcard characters literally.
pub fn search_key(text: &str) -> String {
    text.trim()
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            _ => c,
        })
        .collect()
}
pub fn calendar_year(seconds: i64) -> Result<u16, MessagingError> {
    if !(0..=i64::from(u32::MAX)).contains(&seconds) {
        return Err(MessagingError::Invalid);
    }
    let mut days = seconds / 86400;
    let mut year = 1970;
    loop {
        let length = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if days < length {
            return Ok(year);
        }
        days -= length;
        year += 1;
    }
}
#[derive(Clone, Default)]
pub struct Search {
    pub user: Option<UserId>,
    pub first_name: String,
    pub last_name: String,
    pub nickname: String,
    pub city: String,
    pub gender: Option<Gender>,
    pub years: Option<(u16, u16)>,
    pub active_only: bool,
    pub start: u32,
}
impl Search {
    pub fn validate(&self, year: u16) -> Result<(), MessagingError> {
        for text in [
            &self.first_name,
            &self.last_name,
            &self.nickname,
            &self.city,
        ] {
            validate_text(text)?;
        }
        if self
            .years
            .is_some_and(|(low, high)| low < 1900 || low > high || high > year)
        {
            return Err(MessagingError::Invalid);
        }
        Ok(())
    }
}
pub struct Person {
    pub user: UserId,
    pub profile: Profile,
    pub presence: Presence,
}
pub struct Page {
    pub people: Vec<Person>,
    pub next_start: u32,
}
pub enum Read {
    Own,
    Search(Search),
}
pub enum ReadResult {
    Own(Profile),
    Page(Page),
}
#[async_trait]
pub trait DirectoryRepository: Send + Sync {
    async fn replace_profile(
        &self,
        key: &SessionKey,
        profile: Profile,
        now: i64,
    ) -> Result<(), MessagingError>;
    /// Caller retains the shared publication permit through the resulting transport write.
    async fn read_directory(
        &self,
        key: &SessionKey,
        query: Read,
        now: i64,
    ) -> Result<ReadResult, MessagingError>;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polish_keys_years_and_profile_removal_are_explicit() {
        assert_eq!(search_key("  ŁÓDŹ Żółć  "), "lodz zolc");
        assert_eq!(search_key("_%'"), "_%'");
        assert_eq!(calendar_year(0).unwrap(), 1970);
        assert_eq!(calendar_year(1704067199).unwrap(), 2023);
        assert_eq!(calendar_year(1704067200).unwrap(), 2024);
        let mut p = Profile {
            gender: Some(Gender::Female),
            ..Default::default()
        };
        assert!(p.is_empty());
        p.nickname = "x".into();
        assert!(!p.is_empty());
        p.city = "x\0y".into();
        assert!(p.validate().is_err());
    }
}
