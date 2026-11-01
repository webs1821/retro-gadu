// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
//! One offline import transaction. Protocol decoding belongs to the admin composition layer.
use crate::{SqliteStore, database as sqlx};
use rgg_application::{
    accounts::SealedSecret,
    conferences::ConferenceContext,
    directory::{Gender, Profile, search_key},
    images,
    messaging::{ContactRule, MessageBundle, QueueLimits},
    presence::Presence,
};
use rgg_domain::UserId;
pub struct Account {
    pub user: UserId,
    pub name: String,
    pub email: String,
    pub created: Option<i64>,
    pub secret: SealedSecret,
    pub invitation: Option<[u8; 32]>,
}
pub struct Invitation {
    pub digest: [u8; 32],
    pub used: u32,
    pub active: bool,
    pub created: Option<i64>,
}
pub struct Archive {
    pub owner: UserId,
    pub document: String,
}
pub struct Offline {
    pub owner: UserId,
    pub presence: Presence,
    pub rules: Vec<ContactRule>,
}
pub struct DirectoryEntry {
    pub owner: UserId,
    pub profile: Profile,
}
pub struct Message {
    pub id: i64,
    pub sender: UserId,
    pub bundle: MessageBundle,
    pub accepted: i64,
    pub written: Option<i64>,
}
pub struct Dataset {
    pub source_sha256: String,
    pub package_sha256: String,
    pub now: i64,
    pub pending_ttl: u32,
    pub summary: serde_json::Value,
    pub next_uin: u32,
    pub reserved: Vec<UserId>,
    pub accounts: Vec<Account>,
    pub invitations: Vec<Invitation>,
    pub archives: Vec<Archive>,
    pub offline: Vec<Offline>,
    pub profiles: Vec<DirectoryEntry>,
    pub messages: Vec<Message>,
}
fn failed(_: sqlx::Error) -> &'static str {
    "import transaction failed"
}
pub fn encoded_content(bundle: &MessageBundle) -> Result<(String, String, usize), &'static str> {
    let styles = serde_json::to_string(&bundle.styles).map_err(|_| "invalid imported styles")?;
    let conference = encode_conference(&bundle.conference)?;
    let bytes = bundle.text.as_str().len() + styles.len() + conference.len();
    if bytes > 48 * 1024 {
        return Err("imported message exceeds content limit");
    }
    Ok((styles, conference, bytes))
}
fn encode_conference(c: &ConferenceContext) -> Result<String, &'static str> {
    serde_json::to_string(&c.others().iter().map(|u| u.get()).collect::<Vec<_>>())
        .map_err(|_| "invalid imported conference")
}
impl SqliteStore {
    pub async fn import_dataset(&self, data: &Dataset) -> Result<(), &'static str> {
        let summary = serde_json::to_string(&data.summary).map_err(|_| "invalid import summary")?;
        if summary.len() > 8192 {
            return Err("import summary too large");
        }
        let mut tx = self.pool.begin().await.map_err(failed)?;
        sqlx::query("UPDATE runtime_state SET started_at=started_at WHERE id=1")
            .execute(&mut *tx)
            .await
            .map_err(failed)?;
        let occupied:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM accounts)+(SELECT count(*) FROM messages)+(SELECT count(*) FROM invitations)+(SELECT count(*) FROM legacy_invitations)+(SELECT count(*) FROM retired_uins)+(SELECT count(*) FROM import_receipt)").fetch_one(&mut *tx).await.map_err(failed)?;
        if occupied != 0 {
            return Err("import requires an empty database");
        }
        for row in &data.invitations {
            sqlx::query("INSERT INTO legacy_invitations(digest,max_uses,used,active,created_at) VALUES(?,3,?,?,?)").bind(row.digest.as_slice()).bind(row.used).bind(row.active).bind(row.created.unwrap_or(data.now)).execute(&mut *tx).await.map_err(failed)?;
        }
        for row in &data.accounts {
            sqlx::query("INSERT INTO secret_nonces(key_id,nonce) VALUES(?,?)")
                .bind(&row.secret.key_id)
                .bind(row.secret.nonce.as_slice())
                .execute(&mut *tx)
                .await
                .map_err(failed)?;
            sqlx::query("INSERT INTO accounts(uin,email,secret_version,key_id,nonce,ciphertext,created_at) VALUES(?,?,?,?,?,?,?)")
                .bind(i64::from(row.user.get())).bind(&row.email).bind(i64::from(row.secret.version)).bind(&row.secret.key_id).bind(row.secret.nonce.as_slice()).bind(&row.secret.ciphertext).bind(row.created.unwrap_or(data.now)).execute(&mut *tx).await.map_err(failed)?;
            sqlx::query("INSERT INTO imported_account_metadata(uin,source_name,source_created_at,invite_digest) VALUES(?,?,?,?)")
                .bind(i64::from(row.user.get())).bind(&row.name).bind(row.created).bind(row.invitation.as_ref().map(|v|v.as_slice())).execute(&mut *tx).await.map_err(failed)?;
        }
        for user in &data.reserved {
            sqlx::query("INSERT INTO retired_uins(uin,retired_at) VALUES(?,?)")
                .bind(i64::from(user.get()))
                .bind(data.now)
                .execute(&mut *tx)
                .await
                .map_err(failed)?;
        }
        sqlx::query("UPDATE number_allocator SET next_uin=? WHERE id=1")
            .bind(data.next_uin)
            .execute(&mut *tx)
            .await
            .map_err(failed)?;
        for row in &data.archives {
            sqlx::query(
                "INSERT INTO contact_archives(owner,version,format,document) VALUES(?,1,1,?)",
            )
            .bind(i64::from(row.owner.get()))
            .bind(&row.document)
            .execute(&mut *tx)
            .await
            .map_err(failed)?;
        }
        for row in &data.offline {
            sqlx::query("INSERT INTO account_privacy(uin,revision,friends_only) VALUES(?,1,?)")
                .bind(i64::from(row.owner.get()))
                .bind(row.presence.friends_only)
                .execute(&mut *tx)
                .await
                .map_err(failed)?;
            sqlx::query("INSERT INTO account_presence(uin,availability,description,return_at) VALUES(?,'offline',?,?)").bind(i64::from(row.owner.get())).bind(&row.presence.description).bind(row.presence.return_at).execute(&mut *tx).await.map_err(failed)?;
            for rule in &row.rules {
                sqlx::query("INSERT INTO contact_rules(owner,peer,flags) VALUES(?,?,?)")
                    .bind(i64::from(row.owner.get()))
                    .bind(i64::from(rule.user.get()))
                    .bind(i64::from(rule.flags))
                    .execute(&mut *tx)
                    .await
                    .map_err(failed)?;
            }
        }
        for row in &data.profiles {
            let p = &row.profile;
            let gender = p.gender.map(|g| match g {
                Gender::Female => 1i64,
                Gender::Male => 2,
            });
            sqlx::query("INSERT INTO public_profiles(uin,revision,document,first_key,last_key,nick_key,city_key,family_key,family_city_key,gender,birth_year) VALUES(?,1,?,?,?,?,?,?,?,?,?)")
                .bind(i64::from(row.owner.get())).bind(serde_json::to_string(p).map_err(|_|"invalid import profile")?)
                .bind(search_key(&p.first_name)).bind(search_key(&p.last_name)).bind(search_key(&p.nickname)).bind(search_key(&p.city)).bind(search_key(&p.family_name)).bind(search_key(&p.family_city)).bind(gender).bind(p.birth_year.map(i64::from)).execute(&mut *tx).await.map_err(failed)?;
        }
        for row in &data.messages {
            let b = &row.bundle;
            let (styles, conference, size) = encoded_content(b)?;
            let pending = row.written.is_none();
            sqlx::query("INSERT INTO messages(id,sender,recipient,client_sequence,payload_version,presentation,body,styles,conference,payload_bytes,accepted_at,expires_at) VALUES(?,?,?,?,1,?,?,?,?,?,?,?)")
                .bind(row.id).bind(i64::from(row.sender.get())).bind(i64::from(b.recipient.get())).bind(i64::from(b.client_sequence)).bind(i64::from(b.presentation))
                .bind(pending.then_some(b.text.as_str())).bind(pending.then_some(styles.as_str())).bind(pending.then_some(conference.as_str())).bind(size as i64).bind(row.accepted).bind(data.now+i64::from(data.pending_ttl)).execute(&mut *tx).await.map_err(failed)?;
            sqlx::query("INSERT INTO deliveries(message_id,recipient,enqueue_order,state,terminal_at) VALUES(?,?,?,?,?)")
                .bind(row.id).bind(i64::from(b.recipient.get())).bind(row.id).bind(if pending{"Pending"}else{"TransportWrittenUnconfirmed"}).bind(row.written).execute(&mut *tx).await.map_err(failed)?;
            for image in images::references(&b.styles).map_err(|_| "invalid import images")? {
                let expiry = data.now
                    + if pending {
                        i64::from(data.pending_ttl)
                    } else {
                        images::OFFER_RETENTION_SECONDS
                    };
                sqlx::query("INSERT INTO image_offers(message_id,sender,recipient,size,crc32,expires_at) VALUES(?,?,?,?,?,?)")
                    .bind(row.id).bind(i64::from(row.sender.get())).bind(i64::from(b.recipient.get())).bind(image.size).bind(image.crc32).bind(expiry).execute(&mut *tx).await.map_err(failed)?;
            }
        }
        let limits = QueueLimits::default();
        let quota:i64=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM messages m JOIN deliveries d ON d.message_id=m.id WHERE d.state='Pending' GROUP BY m.recipient HAVING count(*)>? OR sum(payload_bytes)>?) OR EXISTS(SELECT 1 FROM messages m JOIN deliveries d ON d.message_id=m.id WHERE d.state='Pending' GROUP BY m.sender HAVING count(*)>?) OR (SELECT count(*)>? OR COALESCE(sum(payload_bytes),0)>? FROM messages m JOIN deliveries d ON d.message_id=m.id WHERE d.state='Pending')")
            .bind(limits.per_recipient_messages).bind(limits.per_recipient_bytes).bind(limits.per_sender_messages).bind(limits.global_messages).bind(limits.global_bytes).fetch_one(&mut *tx).await.map_err(failed)?;
        if quota != 0 {
            return Err("imported queue exceeds default limits");
        }
        let check: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&mut *tx)
            .await
            .map_err(failed)?;
        if check != "ok"
            || sqlx::query("PRAGMA foreign_key_check")
                .fetch_optional(&mut *tx)
                .await
                .map_err(failed)?
                .is_some()
        {
            return Err("import integrity check failed");
        }
        sqlx::query("INSERT INTO import_receipt(id,source_sha256,package_sha256,imported_at,summary) VALUES(1,?,?,?,?)").bind(&data.source_sha256).bind(&data.package_sha256).bind(data.now).bind(summary).execute(&mut *tx).await.map_err(failed)?;
        #[cfg(feature = "test-hooks")]
        if std::env::var("RGG_TEST_IMPORT_CRASH").as_deref() == Ok("before_import_commit") {
            std::process::exit(86);
        }
        tx.commit().await.map_err(failed)?;
        Ok(())
    }
}
