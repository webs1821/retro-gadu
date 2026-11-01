// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use super::wire;
use crate::Error;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use encoding_rs::WINDOWS_1250;
use rgg_application::{
    accounts::validate_email,
    contacts::validate_archive,
    directory::{Gender, Profile, calendar_year},
    images::{self, ImageId},
    messaging::QueueLimits,
    presence::{Availability, Presence},
};
use rgg_credentials::gg_retro::LegacyKey;
use rgg_domain::UserId;
use rgg_protocol::{
    gg6::encode_text,
    gg6_messages::{contacts, decode_message},
};
use rgg_store_sqlite::import_data as out;
use serde_json::json;
use std::collections::{HashMap, HashSet};
pub fn user(n: u32) -> Result<UserId, Error> {
    if n > 0xffffff {
        return Err("import account number exceeds GG6 range".into());
    }
    Ok(UserId::new(n)?)
}
pub fn hash(value: &str) -> Result<[u8; 32], Error> {
    let mut bytes = [0; 32];
    hex::decode_to_slice(value, &mut bytes).map_err(|_| "invalid import hash")?;
    if hex::encode(bytes) != value {
        return Err("import hash must be canonical lowercase hex".into());
    }
    Ok(bytes)
}
fn blob(value: &str, max: usize) -> Result<Vec<u8>, Error> {
    if value.len() > max.div_ceil(3) * 4 {
        return Err("import binary field exceeds limit".into());
    }
    let bytes = STANDARD
        .decode(value)
        .map_err(|_| "invalid import base64")?;
    if bytes.len() > max {
        return Err("import binary field exceeds limit".into());
    }
    Ok(bytes)
}
fn moment(value: i64) -> Result<(), Error> {
    if !(0..=i64::from(u32::MAX)).contains(&value) {
        return Err("import timestamp outside GG6 range".into());
    }
    Ok(())
}
pub fn prepare(
    mut raw: wire::Package,
    key: &LegacyKey,
    package_hash: String,
    now: i64,
) -> Result<(out::Dataset, Vec<wire::Account>), Error> {
    if raw.format != 1
        || raw.source_commit != "ed9640cc247764754b5e47891053eb07a53f76d5"
        || !raw.missing_history_acknowledged
    {
        return Err("unsupported import source or missing history acknowledgment".into());
    }
    hash(&raw.source_sha256)?;
    moment(now)?;
    if !(1..=30 * 86400).contains(&raw.pending_ttl_seconds)
        || !(112233..=16777216).contains(&raw.next_uin)
    {
        return Err("invalid import allocator or retention policy".into());
    }
    if raw.users.len() > 10000
        || raw.invitations.len() > 10000
        || raw.contacts.len() > 10000
        || raw.presence.len() > 10000
        || raw.profiles.len() > 10000
        || raw.messages.len() > 100000
        || raw.images.len() > images::MAX_OFFERS
        || raw.reserved_uins.len() > 100000
    {
        return Err("import record limit exceeded".into());
    }
    let mut owners = HashSet::new();
    let mut reserved = Vec::new();
    let mut known = HashSet::new();
    for a in &raw.users {
        let u = user(a.uin)?;
        if !owners.insert(a.uin) {
            return Err("duplicate imported account".into());
        }
        validate_email(&a.email)?;
        if a.name.len() > 1024 || a.name.contains('\0') {
            return Err("invalid private account name".into());
        }
        if let Some(t) = a.created_at {
            moment(t)?;
        }
        key.open(u, &a.password)?;
    }
    known.extend(&owners);
    for n in &raw.reserved_uins {
        let u = user(*n)?;
        if !known.insert(*n) {
            return Err("reserved number duplicates an account or reservation".into());
        }
        reserved.push(u);
    }
    let mut invitations = Vec::new();
    let mut invite_ids = HashSet::new();
    for i in raw.invitations {
        let digest = hash(&i.code_hash)?;
        if !invite_ids.insert(digest) || i.max_uses != 3 || i.used > 3 {
            return Err("invalid imported invitation".into());
        }
        if let Some(t) = i.created_at {
            moment(t)?;
        }
        invitations.push(out::Invitation {
            digest,
            used: i.used,
            active: i.active,
            created: i.created_at,
        });
    }
    for a in &raw.users {
        if let Some(code) = &a.registration_invite_hash
            && !invite_ids.contains(&hash(code)?)
        {
            return Err("account references a missing invitation".into());
        }
    }
    let mut seen = HashSet::new();
    let mut archives = Vec::new();
    let mut archive_bytes = 0;
    for c in raw.contacts {
        if !owners.contains(&c.uin) || !seen.insert(c.uin) {
            return Err("invalid contact archive owner".into());
        }
        let bytes = blob(&c.data, 262144)?;
        let (text, bad) = WINDOWS_1250.decode_without_bom_handling(&bytes);
        if bad {
            return Err("contact archive encoding is not representable".into());
        }
        validate_archive(&text)?;
        archive_bytes += text.len();
        archives.push(out::Archive {
            owner: user(c.uin)?,
            document: text.into_owned(),
        });
    }
    if archive_bytes > 96 * 1024 * 1024 {
        return Err("imported contact archives exceed global limit".into());
    }
    seen.clear();
    let mut offline = Vec::new();
    let mut blocks = HashSet::new();
    for p in raw.presence {
        if !owners.contains(&p.uin)
            || !seen.insert(p.uin)
            || ![0x15, 0x8015].contains(&p.status)
            || p.description.is_empty()
        {
            return Err("invalid imported offline presence".into());
        }
        encode_text(&p.description)?;
        let presence = Presence {
            availability: Availability::Offline,
            description: Some(p.description),
            return_at: p.return_time,
            friends_only: p.status & 0x8000 != 0,
        };
        presence.validate()?;
        let encoded = blob(&p.privacy, 5000)?;
        let mut rules = Vec::new();
        let mut peers = HashSet::new();
        for chunk in encoded.chunks(2000) {
            for rule in contacts(chunk)? {
                if !peers.insert(rule.user.get()) {
                    return Err("duplicate imported privacy peer".into());
                }
                if rule.flags & 4 != 0 {
                    blocks.insert((p.uin, rule.user.get()));
                }
                rules.push(rule);
            }
        }
        offline.push(out::Offline {
            owner: user(p.uin)?,
            presence,
            rules,
        });
    }
    seen.clear();
    let mut profiles = Vec::new();
    for p in raw.profiles {
        if !owners.contains(&p.uin) || !seen.insert(p.uin) {
            return Err("invalid imported profile owner".into());
        }
        let profile = Profile {
            first_name: p.firstname,
            last_name: p.lastname,
            nickname: p.nickname,
            city: p.city,
            family_name: p.familyname,
            family_city: p.familycity,
            gender: match p.gender {
                0 => None,
                1 => Some(Gender::Male),
                2 => Some(Gender::Female),
                _ => return Err("invalid imported gender".into()),
            },
            birth_year: (p.birthyear != 0).then_some(p.birthyear),
        };
        profile.validate()?;
        if profile.is_empty()
            || profile
                .birth_year
                .is_some_and(|y| y > calendar_year(now).unwrap_or(0))
        {
            return Err("empty or future-dated source profile".into());
        }
        for s in [
            &profile.first_name,
            &profile.last_name,
            &profile.nickname,
            &profile.city,
            &profile.family_name,
            &profile.family_city,
        ] {
            encode_text(s)?;
        }
        profiles.push(out::DirectoryEntry {
            owner: user(p.uin)?,
            profile,
        });
    }
    let mut offers: HashMap<i64, HashSet<ImageId>> = HashMap::new();
    for i in raw.images {
        let image = ImageId {
            size: i.size,
            crc32: i.crc32,
        };
        image.validate()?;
        if !offers.entry(i.message_id).or_default().insert(image) {
            return Err("duplicate imported image offer".into());
        }
    }
    let mut messages = Vec::new();
    let mut ids = HashSet::new();
    let mut pending = 0u32;
    let mut total_bytes = 0usize;
    let mut recipients: HashMap<u32, (u32, usize)> = HashMap::new();
    let mut senders: HashMap<u32, u32> = HashMap::new();
    let mut offer_users: HashMap<u32, usize> = HashMap::new();
    for m in raw.messages {
        if m.id <= 0 || m.id >= i64::MAX - 1 || !ids.insert(m.id) {
            return Err("invalid or duplicate message id".into());
        }
        if !known.contains(&m.sender_uin) || !known.contains(&m.recipient_uin) {
            return Err(
                "message participant requires an account or explicit reserved number".into(),
            );
        }
        if m.delivered_at.is_none()
            && (!owners.contains(&m.sender_uin) || !owners.contains(&m.recipient_uin))
        {
            return Err("pending message references a missing account".into());
        }
        moment(m.time)?;
        if let Some(t) = m.delivered_at {
            moment(t)?;
        }
        let mut packet = Vec::new();
        packet.extend(m.recipient_uin.to_le_bytes());
        packet.extend(m.seq.to_le_bytes());
        packet.extend(m.class.to_le_bytes());
        packet.extend(encode_text(&m.message)?);
        packet.push(0);
        for extra in [&m.formatting, &m.conference].into_iter().flatten() {
            packet.extend(blob(extra, 4096)?);
        }
        let bundle = decode_message(&packet)?;
        let sender = user(m.sender_uin)?;
        bundle.conference.validate_for(sender, bundle.recipient)?;
        let references: HashSet<_> = images::references(&bundle.styles)?.into_iter().collect();
        if references != offers.remove(&m.id).unwrap_or_default() {
            return Err("image offers do not match message references".into());
        }
        if !references.is_empty() {
            if m.sender_uin == m.recipient_uin
                || !owners.contains(&m.sender_uin)
                || !owners.contains(&m.recipient_uin)
            {
                return Err("image offer references unavailable participants".into());
            }
            for id in [m.sender_uin, m.recipient_uin] {
                *offer_users.entry(id).or_default() += references.len();
            }
        }
        if (m.delivered_at.is_none() || !references.is_empty())
            && (blocks.contains(&(m.sender_uin, m.recipient_uin))
                || blocks.contains(&(m.recipient_uin, m.sender_uin)))
        {
            return Err(
                "pending content conflicts with saved privacy; explicit source resolution required"
                    .into(),
            );
        }
        if m.delivered_at.is_none() {
            let size = out::encoded_content(&bundle)?.2;
            pending += 1;
            total_bytes += size;
            let r = recipients.entry(m.recipient_uin).or_default();
            r.0 += 1;
            r.1 += size;
            *senders.entry(m.sender_uin).or_default() += 1;
        }
        messages.push(out::Message {
            id: m.id,
            sender,
            bundle,
            accepted: m.time,
            written: m.delivered_at,
        });
    }
    if !offers.is_empty() {
        return Err("orphan imported image offer".into());
    }
    let limits = QueueLimits::default();
    if pending > limits.global_messages
        || total_bytes > limits.global_bytes as usize
        || recipients.values().any(|(n, b)| {
            *n > limits.per_recipient_messages || *b > limits.per_recipient_bytes as usize
        })
        || senders.values().any(|n| *n > limits.per_sender_messages)
        || offer_users
            .values()
            .any(|n| *n > images::MAX_OFFERS_PER_USER)
    {
        return Err("imported queue or offers exceed default limits".into());
    }
    messages.sort_by_key(|m| m.id);
    let summary = json!({"accounts":raw.users.len(),"invitations":invitations.len(),"invitation_used":invitations.iter().map(|v|u64::from(v.used)).sum::<u64>(),"reserved_uins":reserved.len(),"archives":archives.len(),"saved_offline_presence":offline.len(),"accounts_without_saved_privacy":raw.users.len()-offline.len(),"profiles":profiles.len(),"pending_messages":pending,"terminal_message_headers":messages.len()-pending as usize,"image_offers":offer_users.values().sum::<usize>()/2,"discarded_transient_tokens":raw.discarded_tokens,"pending_ttl_renewed_seconds":raw.pending_ttl_seconds,"delivered_offer_retention_renewed_seconds":images::OFFER_RETENTION_SECONDS,"missing_history_acknowledged":true,"identifier_mapping":"uin/message_id preserved","missing_account_creation_times":raw.users.iter().filter(|a|a.created_at.is_none()).count()});
    let data = out::Dataset {
        source_sha256: raw.source_sha256,
        package_sha256: package_hash,
        now,
        pending_ttl: raw.pending_ttl_seconds,
        summary,
        next_uin: raw.next_uin,
        reserved,
        accounts: Vec::new(),
        invitations,
        archives,
        offline,
        profiles,
        messages,
    };
    Ok((data, std::mem::take(&mut raw.users)))
}
