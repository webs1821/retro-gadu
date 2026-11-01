// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! Private exchange format produced from a verified, offline source snapshot.
use serde::Deserialize;
use zeroize::Zeroizing;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub format: u32,
    pub source_commit: String,
    pub source_sha256: String,
    pub missing_history_acknowledged: bool,
    pub pending_ttl_seconds: u32,
    pub reserved_uins: Vec<u32>,
    pub next_uin: u32,
    pub discarded_tokens: u64,
    pub users: Vec<Account>,
    pub invitations: Vec<Invitation>,
    pub contacts: Vec<Contacts>,
    pub presence: Vec<Offline>,
    pub profiles: Vec<Profile>,
    pub messages: Vec<Message>,
    pub images: Vec<Image>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub uin: u32,
    pub name: String,
    pub email: String,
    pub password: Zeroizing<String>,
    pub created_at: Option<i64>,
    pub registration_invite_hash: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invitation {
    pub code_hash: String,
    pub max_uses: u32,
    pub used: u32,
    pub active: bool,
    pub created_at: Option<i64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contacts {
    pub uin: u32,
    pub data: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offline {
    pub uin: u32,
    pub status: u32,
    pub description: String,
    pub return_time: Option<u32>,
    pub privacy: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub uin: u32,
    pub firstname: String,
    pub lastname: String,
    pub nickname: String,
    pub city: String,
    pub familyname: String,
    pub familycity: String,
    pub birthyear: u16,
    pub gender: u8,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub id: i64,
    pub sender_uin: u32,
    pub recipient_uin: u32,
    pub seq: u32,
    pub time: i64,
    pub class: u32,
    pub message: String,
    pub delivered_at: Option<i64>,
    pub formatting: Option<String>,
    pub conference: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub message_id: i64,
    pub size: u32,
    pub crc32: u32,
}
