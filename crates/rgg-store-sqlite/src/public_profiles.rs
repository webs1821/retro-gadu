// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
use crate::{
    SqliteStore, database as sqlx,
    messaging::{db, fence},
    presence::record_presence,
};
use async_trait::async_trait;
use futures_util::TryStreamExt;
use rgg_application::{
    directory::*,
    messaging::{MessagingError, SessionKey},
    presence::{Availability, effective_presence_for},
};
use rgg_domain::UserId;
use sqlx::Row;
fn gender(value: Option<Gender>) -> Option<i64> {
    value.map(|v| if v == Gender::Female { 1 } else { 2 })
}
fn profile(document: &str) -> Result<Profile, MessagingError> {
    if document.len() > 4096 {
        return Err(MessagingError::Integrity);
    }
    let p: Profile = serde_json::from_str(document).map_err(|_| MessagingError::Integrity)?;
    p.validate().map_err(|_| MessagingError::Integrity)?;
    Ok(p)
}
async fn admit(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    key: &SessionKey,
    now: i64,
) -> Result<(), MessagingError> {
    calendar_year(now)?;
    fence(tx, key, true).await?;
    // Persistent per-account budget; reconnect and process restart do not reset it.
    // Backwards UTC movement conservatively postpones renewal.
    let used:Option<i64>=sqlx::query_scalar("INSERT INTO directory_rate(uin,window,used) VALUES(?,?,1) ON CONFLICT(uin) DO UPDATE SET used=CASE WHEN ?-window>=10 THEN 1 ELSE used+1 END,window=CASE WHEN ?-window>=10 THEN ? ELSE window END WHERE used<20 OR ?-window>=10 RETURNING used")
        .bind(i64::from(key.user.get())).bind(now).bind(now).bind(now).bind(now).bind(now).fetch_optional(&mut **tx).await.map_err(db)?;
    used.ok_or(MessagingError::Quota).map(|_| ())
}
#[async_trait]
impl DirectoryRepository for SqliteStore {
    async fn replace_profile(
        &self,
        key: &SessionKey,
        p: Profile,
        now: i64,
    ) -> Result<(), MessagingError> {
        p.validate()?;
        if p.birth_year
            .is_some_and(|y| y > calendar_year(now).unwrap_or(0))
        {
            return Err(MessagingError::Invalid);
        }
        let mut tx = self.writer.begin().await.map_err(db)?;
        admit(&mut tx, key, now).await?;
        if p.is_empty() {
            sqlx::query("DELETE FROM public_profiles WHERE uin=?")
                .bind(i64::from(key.user.get()))
                .execute(&mut *tx)
                .await
                .map_err(db)?;
        } else {
            let capacity:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM public_profiles)<? OR EXISTS(SELECT 1 FROM public_profiles WHERE uin=?)").bind(MAX_PROFILES).bind(i64::from(key.user.get())).fetch_one(&mut *tx).await.map_err(db)?;
            if capacity == 0 {
                return Err(MessagingError::Quota);
            }
            let changed=sqlx::query("INSERT INTO public_profiles(uin,revision,document,first_key,last_key,nick_key,city_key,family_key,family_city_key,gender,birth_year) VALUES(?,1,?,?,?,?,?,?,?,?,?) ON CONFLICT(uin) DO UPDATE SET revision=revision+1,document=excluded.document,first_key=excluded.first_key,last_key=excluded.last_key,nick_key=excluded.nick_key,city_key=excluded.city_key,family_key=excluded.family_key,family_city_key=excluded.family_city_key,gender=excluded.gender,birth_year=excluded.birth_year WHERE revision<9223372036854775807")
                .bind(i64::from(key.user.get())).bind(serde_json::to_string(&p).map_err(|_|MessagingError::Invalid)?)
                .bind(search_key(&p.first_name)).bind(search_key(&p.last_name)).bind(search_key(&p.nickname)).bind(search_key(&p.city)).bind(search_key(&p.family_name)).bind(search_key(&p.family_city)).bind(gender(p.gender)).bind(p.birth_year.map(i64::from)).execute(&mut *tx).await.map_err(db)?.rows_affected();
            if changed != 1 {
                return Err(MessagingError::Quota);
            }
        }
        tx.commit().await.map_err(db)
    }
    async fn read_directory(
        &self,
        key: &SessionKey,
        request: Read,
        now: i64,
    ) -> Result<ReadResult, MessagingError> {
        if let Read::Search(ref query) = request {
            query.validate(calendar_year(now)?)?;
        }
        let mut tx = self.writer.begin().await.map_err(db)?;
        admit(&mut tx, key, now).await?;
        let result = match request {
            Read::Own => {
                let document: Option<String> =
                    sqlx::query_scalar("SELECT document FROM public_profiles WHERE uin=?")
                        .bind(i64::from(key.user.get()))
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(db)?;
                ReadResult::Own(
                    document
                        .as_deref()
                        .map(profile)
                        .transpose()?
                        .unwrap_or_default(),
                )
            }
            Read::Search(query) => {
                let (low, high) = query.years.unwrap_or((0, 0));
                let requested = query.user.map(|u| i64::from(u.get()));
                let sex = gender(query.gender);
                let last = search_key(&query.last_name);
                let city = search_key(&query.city);
                let mut rows=sqlx::query("SELECT d.uin,d.document,COALESCE(policy.flags,0) subject_flags,COALESCE(viewer.flags,0) viewer_flags,COALESCE(priv.friends_only,0) friends_only,p.availability,p.description,p.return_at,CASE WHEN p.availability='offline' AND p.session_id IS NULL THEN 1 WHEN s.ready=1 AND s.epoch=? AND s.session_id=p.session_id AND a.active=1 AND a.credential_version=s.credential_version AND a.session_generation=s.generation THEN 1 ELSE 0 END visible FROM public_profiles d JOIN accounts a ON a.uin=d.uin AND a.active=1 LEFT JOIN account_presence p ON p.uin=d.uin LEFT JOIN account_sessions s ON s.uin=d.uin LEFT JOIN account_privacy priv ON priv.uin=d.uin LEFT JOIN contact_rules policy ON policy.owner=d.uin AND policy.peer=? LEFT JOIN contact_rules viewer ON viewer.owner=? AND viewer.peer=d.uin WHERE d.uin>=? AND (? IS NULL OR d.uin=?) AND instr(d.first_key,?)>0 AND (instr(d.last_key,?)>0 OR instr(d.family_key,?)>0) AND instr(d.nick_key,?)>0 AND (instr(d.city_key,?)>0 OR instr(d.family_city_key,?)>0) AND (? IS NULL OR d.gender=?) AND (?=0 OR d.birth_year BETWEEN ? AND ?) ORDER BY d.uin LIMIT 10001")
                    .bind(&key.epoch).bind(i64::from(key.user.get())).bind(i64::from(key.user.get())).bind(i64::from(query.start)).bind(requested).bind(requested).bind(search_key(&query.first_name)).bind(&last).bind(&last).bind(search_key(&query.nickname)).bind(&city).bind(&city).bind(sex).bind(sex).bind(i64::from(low)).bind(i64::from(low)).bind(i64::from(high)).fetch(&mut *tx);
                let mut page = Page {
                    people: Vec::with_capacity(PAGE_SIZE),
                    next_start: 0,
                };
                let mut scanned = 0;
                while let Some(row) = rows.try_next().await.map_err(db)? {
                    scanned += 1;
                    if scanned > MAX_PROFILES {
                        return Err(MessagingError::Quota);
                    }
                    let visible = effective_presence_for(
                        &record_presence(&row)?,
                        row.get::<i64, _>("subject_flags") as u8,
                        row.get::<i64, _>("viewer_flags") as u8,
                    );
                    if query.active_only
                        && !matches!(
                            visible.availability,
                            Availability::Available | Availability::Away
                        )
                    {
                        continue;
                    }
                    let user = UserId::new(
                        u32::try_from(row.get::<i64, _>("uin"))
                            .map_err(|_| MessagingError::Integrity)?,
                    )
                    .map_err(|_| MessagingError::Integrity)?;
                    if page.people.len() == PAGE_SIZE {
                        page.next_start = user.get();
                        break;
                    }
                    page.people.push(Person {
                        user,
                        profile: profile(row.get::<&str, _>("document"))?,
                        presence: visible,
                    });
                }
                drop(rows);
                ReadResult::Page(page)
            }
        };
        tx.commit().await.map_err(db)?;
        Ok(result)
    }
}
