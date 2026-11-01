// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::messaging::MessagingError;
use rgg_domain::UserId;

pub const MAX_PARTICIPANTS: usize = 32;
pub const MAX_OTHER_PARTICIPANTS: usize = MAX_PARTICIPANTS - 2;

/// Sender-declared conversation context, never an instruction to fan out.
/// The primary recipient and authenticated sender are deliberately not in this list.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ConferenceContext {
    others: Vec<UserId>,
}
impl std::fmt::Debug for ConferenceContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConferenceContext")
            .field("other_participants", &self.others.len())
            .finish()
    }
}
impl ConferenceContext {
    pub fn new(others: Vec<UserId>) -> Result<Self, MessagingError> {
        if others.len() > MAX_OTHER_PARTICIPANTS
            || others
                .iter()
                .enumerate()
                .any(|(i, u)| others[..i].contains(u))
        {
            return Err(MessagingError::Invalid);
        }
        Ok(Self { others })
    }
    pub fn others(&self) -> &[UserId] {
        &self.others
    }
    pub fn validate_for(&self, sender: UserId, recipient: UserId) -> Result<(), MessagingError> {
        if !self.others.is_empty()
            && (sender == recipient
                || self.others.contains(&sender)
                || self.others.contains(&recipient))
        {
            Err(MessagingError::Invalid)
        } else {
            Ok(())
        }
    }
}
