// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Image references are identifiers, never bearer credentials or filesystem paths.
use crate::messaging::{MessagingError, SessionKey, TextStyle};
use async_trait::async_trait;
use rgg_domain::UserId;
use serde::{Deserialize, Serialize};

pub const MAX_IMAGE_BYTES: u32 = 255 * 1024;
pub const MAX_IMAGES_PER_MESSAGE: usize = 8;
pub const MAX_CHUNK_BYTES: usize = 1900;
pub const OFFER_RETENTION_SECONDS: i64 = 7 * 86400;
pub const MAX_OFFERS: usize = 80000;
pub const MAX_OFFERS_PER_USER: usize = 8000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageId {
    pub size: u32,
    pub crc32: u32,
}
impl ImageId {
    pub fn validate(self) -> Result<(), MessagingError> {
        if (1..=MAX_IMAGE_BYTES).contains(&self.size) {
            Ok(())
        } else {
            Err(MessagingError::Invalid)
        }
    }
}
pub fn references(styles: &[TextStyle]) -> Result<Vec<ImageId>, MessagingError> {
    let mut images = Vec::new();
    for image in styles.iter().filter_map(|s| s.image) {
        image.validate()?;
        if !images.contains(&image) {
            images.push(image);
        }
        if images.len() > MAX_IMAGES_PER_MESSAGE {
            return Err(MessagingError::Invalid);
        }
    }
    Ok(images)
}
#[derive(Clone, PartialEq, Eq)]
pub enum ImageControl {
    Request(ImageId),
    Missing(ImageId),
    Start {
        image: ImageId,
        filename: Vec<u8>,
        data: Vec<u8>,
    },
    Continue {
        image: ImageId,
        data: Vec<u8>,
    },
}
impl std::fmt::Debug for ImageControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageControl")
            .field("image", &self.image())
            .finish_non_exhaustive()
    }
}
impl ImageControl {
    pub fn image(&self) -> ImageId {
        match self {
            Self::Request(i) | Self::Missing(i) => *i,
            Self::Start { image, .. } | Self::Continue { image, .. } => *image,
        }
    }
    pub fn validate(&self) -> Result<(), MessagingError> {
        self.image().validate()?;
        match self {
            Self::Start { filename, data, .. }
                if filename.is_empty()
                    || filename.len() > 1024
                    || filename
                        .iter()
                        .any(|b| *b < 32 || [127, b'/', b'\\', b':'].contains(b))
                    || filename == b"."
                    || filename == b".."
                    || filename.len() + 1 + data.len() > MAX_CHUNK_BYTES
                    || data.len() > self.image().size as usize =>
            {
                Err(MessagingError::Invalid)
            }
            Self::Continue { data, .. }
                if data.is_empty()
                    || data.len() > MAX_CHUNK_BYTES
                    || data.len() > self.image().size as usize =>
            {
                Err(MessagingError::Invalid)
            }
            _ => Ok(()),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRoute {
    pub sender: SessionKey,
    pub recipient: SessionKey,
    pub image: ImageId,
}

#[async_trait]
pub trait ImageRepository: Send + Sync {
    async fn set_image_limit(&self, key: &SessionKey, kib: u8) -> Result<(), MessagingError>;
    /// Current Ready sessions, durable recipient-specific offer, both block policies and limit.
    async fn authorize_image(
        &self,
        requester: &SessionKey,
        sender: UserId,
        image: ImageId,
        now: i64,
    ) -> Result<ImageRoute, MessagingError>;
    async fn image_route_current(
        &self,
        route: &ImageRoute,
        now: i64,
    ) -> Result<bool, MessagingError>;
}
pub struct ImageAction {
    pub route: ImageRoute,
    pub to_sender: bool,
    pub control: ImageControl,
}
pub const TRANSFER_STATES: [&str; 4] = ["requested", "collecting", "ready", "missing"];
/// A bounded, read-only view of coordinator-owned buffers, not total process memory.
#[derive(Clone, Copy, Debug, Default)]
pub struct ImageSnapshot {
    pub transfers: [u64; 4],
    pub reserved_bytes: u64,
    pub buffered_bytes: u64,
    pub buffer_capacity_bytes: u64,
    pub oldest_started: Option<std::time::Instant>,
}
pub trait ImageObservations: Send + Sync {
    /// None means unavailable (including lock contention), never an invented empty state.
    /// Must not allocate, wait for a busy lock, expire transfers or advance their state.
    fn image_snapshot(&self) -> Option<ImageSnapshot>;
}
/// Synchronous bounded coordinator. The caller holds the publication gate during each call
/// and revalidates the route before every socket write. No disk or network I/O in this port.
pub trait ImageTransfers: Send + Sync {
    fn request(&self, route: ImageRoute) -> Result<(), MessagingError>;
    fn reply(
        &self,
        source: &SessionKey,
        recipient: UserId,
        control: ImageControl,
    ) -> Result<(), MessagingError>;
    fn next(&self, destination: &SessionKey) -> Option<ImageAction>;
    fn cancel(&self, route: &ImageRoute);
    fn disconnect(&self, key: &SessionKey);
}
