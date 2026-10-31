// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821

//! Shared concepts. This crate knows nothing about TCP, HTTP, SQLite or GG packet layouts.

use std::{fmt, num::NonZeroU32};
use thiserror::Error;

pub const MAX_TEXT_BYTES: usize = 16 * 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("user number must be nonzero")]
    ZeroUser,
    #[error("message must contain text")]
    EmptyText,
    #[error("message contains a NUL character")]
    EmbeddedNul,
    #[error("message exceeds the internal text limit")]
    TextTooLong,
}

/// The core accepts the full nonzero u32 range; adapters apply client-specific limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UserId(NonZeroU32);

impl UserId {
    pub fn new(value: u32) -> Result<Self, ValidationError> {
        NonZeroU32::new(value)
            .map(Self)
            .ok_or(ValidationError::ZeroUser)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MessageText(String);

impl MessageText {
    pub fn new(text: String) -> Result<Self, ValidationError> {
        if text.is_empty() {
            return Err(ValidationError::EmptyText);
        }
        if text.len() > MAX_TEXT_BYTES {
            return Err(ValidationError::TextTooLong);
        }
        if text.contains('\0') {
            return Err(ValidationError::EmbeddedNul);
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for MessageText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MessageText([redacted])")
    }
}

/// Text-only foundation. Rich text, images and conferences require explicit future fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextMessage {
    pub sender: UserId,
    pub recipient: UserId,
    pub client_sequence: u32,
    pub text: MessageText,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_ids_do_not_inherit_a_legacy_wire_limit() {
        assert_eq!(UserId::new(0), Err(ValidationError::ZeroUser));
        assert_eq!(UserId::new(u32::MAX).unwrap().get(), u32::MAX);
    }

    #[test]
    fn text_validation_and_debug_do_not_leak_content() {
        assert_eq!(
            MessageText::new(String::new()),
            Err(ValidationError::EmptyText)
        );
        assert_eq!(
            MessageText::new("a\0b".into()),
            Err(ValidationError::EmbeddedNul)
        );
        assert_eq!(
            MessageText::new("ą".repeat(MAX_TEXT_BYTES)),
            Err(ValidationError::TextTooLong)
        );
        let text = MessageText::new("Zażółć gęślą jaźń".into()).unwrap();
        assert_eq!(text.as_str(), "Zażółć gęślą jaźń");
        assert!(!format!("{text:?}").contains("Zażółć"));
    }
}
