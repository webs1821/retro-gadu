// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::messaging::{ContactRule, MessagingError, SessionKey};
use async_trait::async_trait;
use rgg_domain::UserId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    Offline,
    Available,
    Away,
    Invisible,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Presence {
    pub availability: Availability,
    pub description: Option<String>,
    pub return_at: Option<u32>,
    pub friends_only: bool,
}
impl std::fmt::Debug for Presence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Presence")
            .field("availability", &self.availability)
            .field("description", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}
impl Presence {
    pub fn offline() -> Self {
        Self {
            availability: Availability::Offline,
            description: None,
            return_at: None,
            friends_only: false,
        }
    }
    pub fn validate(&self) -> Result<(), MessagingError> {
        if self
            .description
            .as_ref()
            .is_some_and(|d| d.len() > 210 || d.chars().count() > 70 || d.contains('\0'))
            || (self.return_at.is_some() && self.description.is_none())
        {
            Err(MessagingError::Invalid)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisiblePresence {
    pub user: UserId,
    pub presence: Presence,
}
/// The subject's policy controls disclosure; a viewer may additionally block the subject.
pub fn effective_presence_for(presence: &Presence, subject_rule: u8, viewer_rule: u8) -> Presence {
    if presence.availability == Availability::Invisible
        || subject_rule & 4 != 0
        || subject_rule & 3 == 1
        || viewer_rule & 4 != 0
        || (presence.friends_only && subject_rule & 2 == 0)
    {
        Presence::offline()
    } else {
        let mut visible = presence.clone();
        visible.friends_only = false;
        visible
    }
}
#[async_trait]
pub trait PresenceRepository: Send + Sync {
    async fn activate_presence(
        &self,
        key: &SessionKey,
        rules: &[ContactRule],
        presence: &Presence,
        now: i64,
    ) -> Result<(), MessagingError>;
    async fn set_presence(
        &self,
        key: &SessionKey,
        presence: &Presence,
    ) -> Result<(), MessagingError>;
    /// Caller holds the shared publication gate through the eventual socket write.
    async fn presence_snapshot(
        &self,
        key: &SessionKey,
    ) -> Result<Vec<VisiblePresence>, MessagingError>;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn privacy_hides_the_entire_status_and_never_exports_invisibility() {
        let p = Presence {
            availability: Availability::Available,
            description: Some("private".into()),
            return_at: Some(123),
            friends_only: false,
        };
        for (subject, viewer) in [(4, 3), (1, 3), (5, 3), (3, 7)] {
            assert_eq!(
                effective_presence_for(&p, subject, viewer),
                Presence::offline()
            );
        }
        let mut p = p;
        p.friends_only = true;
        assert_eq!(effective_presence_for(&p, 0, 3), Presence::offline());
        assert_eq!(
            effective_presence_for(&p, 3, 3).description.as_deref(),
            Some("private")
        );
        p.availability = Availability::Invisible;
        assert_eq!(effective_presence_for(&p, 3, 3), Presence::offline());
        assert!(!format!("{p:?}").contains("private"));
    }
}
