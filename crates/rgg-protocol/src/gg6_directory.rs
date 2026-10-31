// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 webs1821
// Independent wire implementation: libgadu-wire-reference and gg-retro-compatibility-reference.
// Native GG6.1 observations, not copied code: docs/implementation/PUBLIC-DIRECTORY.md.
use crate::{
    frame::Frame,
    gg6::{DecodeError, encode_text},
};
use bytes::Bytes;
use encoding_rs::WINDOWS_1250;
use rgg_application::{
    directory::*,
    presence::{Availability, Presence},
};
use rgg_domain::UserId;
use std::collections::BTreeMap;
pub enum Operation {
    Write(Profile),
    Read(Read),
}
pub struct Request {
    pub kind: u8,
    pub sequence: u32,
    pub operation: Operation,
}
pub fn header(body: &[u8]) -> Option<(u8, u32)> {
    (body.len() >= 5 && (1..=3).contains(&body[0])).then(|| {
        (
            body[0],
            u32::from_le_bytes(body[1..5].try_into().expect("bounded header")),
        )
    })
}
fn number(value: &str) -> Result<u32, DecodeError> {
    if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Err(DecodeError::Length);
    }
    value.parse().map_err(|_| DecodeError::Length)
}
fn value<'a>(fields: &'a BTreeMap<String, String>, name: &str) -> &'a str {
    fields.get(name).map(String::as_str).unwrap_or("")
}
fn sex(text: &str, own: bool) -> Result<Option<Gender>, DecodeError> {
    match text {
        "" | "0" => Ok(None),
        "1" => Ok(Some(if own { Gender::Male } else { Gender::Female })),
        "2" => Ok(Some(if own { Gender::Female } else { Gender::Male })),
        _ => Err(DecodeError::Length),
    }
}
pub fn decode(body: &[u8]) -> Result<Request, DecodeError> {
    let (kind, sequence) = header(body).ok_or(DecodeError::Length)?;
    if body.len() > 4096 {
        return Err(DecodeError::Length);
    }
    let mut rest = &body[5..];
    let mut fields = BTreeMap::new();
    while !rest.is_empty() {
        let at = rest
            .iter()
            .position(|b| *b == 0)
            .ok_or(DecodeError::Length)?;
        if at == 0 || at > 24 || !rest[..at].is_ascii() {
            return Err(DecodeError::Length);
        }
        let mut key = std::str::from_utf8(&rest[..at])
            .map_err(|_| DecodeError::Encoding)?
            .to_ascii_lowercase();
        if key == "start" {
            key = "fmstart".into();
        }
        rest = &rest[at + 1..];
        let at = rest
            .iter()
            .position(|b| *b == 0)
            .ok_or(DecodeError::Length)?;
        if at > 60 {
            return Err(DecodeError::Length);
        }
        let (text, invalid) = WINDOWS_1250.decode_without_bom_handling(&rest[..at]);
        if invalid || text.chars().any(char::is_control) {
            return Err(DecodeError::Encoding);
        }
        if fields.insert(key, text.trim().to_owned()).is_some() || fields.len() > 16 {
            return Err(DecodeError::Length);
        }
        rest = &rest[at + 1..];
    }
    let common = [
        "firstname",
        "lastname",
        "nickname",
        "city",
        "gender",
        "birthyear",
    ];
    for key in fields.keys() {
        let allowed = match kind {
            1 => {
                common.contains(&key.as_str())
                    || ["familyname", "familycity"].contains(&key.as_str())
            }
            2 => false,
            3 => {
                common.contains(&key.as_str())
                    || ["fmnumber", "fmstart", "activeonly"].contains(&key.as_str())
            }
            _ => false,
        };
        if !allowed {
            return Err(DecodeError::Length);
        }
    }
    let text = |key| value(&fields, key).to_owned();
    let operation = match kind {
        1 => {
            let year = match value(&fields, "birthyear") {
                "" | "0" => None,
                v => Some(u16::try_from(number(v)?).map_err(|_| DecodeError::Length)?),
            };
            let p = Profile {
                first_name: text("firstname"),
                last_name: text("lastname"),
                nickname: text("nickname"),
                city: text("city"),
                family_name: text("familyname"),
                family_city: text("familycity"),
                gender: sex(value(&fields, "gender"), true)?,
                birth_year: year,
            };
            p.validate().map_err(|_| DecodeError::Length)?;
            Operation::Write(p)
        }
        2 => Operation::Read(Read::Own),
        3 => {
            if sequence == 0 {
                return Err(DecodeError::Length);
            }
            let user = match value(&fields, "fmnumber") {
                "" | "0" => None,
                v => {
                    let n = number(v)?;
                    if n > 0x00ff_ffff {
                        return Err(DecodeError::UserNumber);
                    }
                    Some(UserId::new(n).map_err(|_| DecodeError::UserNumber)?)
                }
            };
            let years = match value(&fields, "birthyear") {
                "" | "0" => None,
                v => {
                    let years = v
                        .split_ascii_whitespace()
                        .map(number)
                        .collect::<Result<Vec<_>, _>>()?;
                    if !(1..=2).contains(&years.len()) {
                        return Err(DecodeError::Length);
                    }
                    let a = u16::try_from(years[0]).map_err(|_| DecodeError::Length)?;
                    let b = u16::try_from(*years.last().expect("nonempty"))
                        .map_err(|_| DecodeError::Length)?;
                    Some((a, b))
                }
            };
            let active_only = match value(&fields, "activeonly") {
                "" | "0" => false,
                "1" => true,
                _ => return Err(DecodeError::Length),
            };
            let start = match value(&fields, "fmstart") {
                "" => 0,
                v => number(v)?,
            };
            if start > 0x00ff_ffff {
                return Err(DecodeError::UserNumber);
            }
            Operation::Read(Read::Search(Search {
                user,
                first_name: text("firstname"),
                last_name: text("lastname"),
                nickname: text("nickname"),
                city: text("city"),
                gender: sex(value(&fields, "gender"), false)?,
                years,
                active_only,
                start,
            }))
        }
        _ => return Err(DecodeError::Length),
    };
    Ok(Request {
        kind,
        sequence,
        operation,
    })
}
fn prefix(kind: u8, sequence: u32) -> Vec<u8> {
    let mut out = vec![kind];
    out.extend(sequence.to_le_bytes());
    out
}
fn frame(body: Vec<u8>) -> Frame {
    Frame {
        kind: 0xe,
        payload: Bytes::from(body),
    }
}
fn pair(out: &mut Vec<u8>, key: &str, text: &str) -> Result<(), DecodeError> {
    out.extend(key.as_bytes());
    out.push(0);
    out.extend(encode_text(text)?);
    out.push(0);
    Ok(())
}
pub fn failure(request: u8, sequence: u32) -> Frame {
    frame(prefix(
        match request {
            1 => 2,
            2 => 4,
            _ => 6,
        },
        sequence,
    ))
}
pub fn written(sequence: u32) -> Frame {
    frame(prefix(1, sequence))
}
fn status(p: &Presence) -> u32 {
    match (p.availability, p.description.is_some()) {
        (Availability::Available, false) => 2,
        (Availability::Available, true) => 4,
        (Availability::Away, false) => 3,
        (Availability::Away, true) => 5,
        (Availability::Offline, true) => 0x15,
        _ => 1,
    }
}
pub fn result(sequence: u32, user: UserId, result: &ReadResult) -> Result<Frame, DecodeError> {
    let mut out = prefix(
        if matches!(result, ReadResult::Own(_)) {
            3
        } else {
            5
        },
        sequence,
    );
    match result {
        ReadResult::Own(p) => {
            p.validate().map_err(|_| DecodeError::Length)?;
            if user.get() > 0x00ff_ffff {
                return Err(DecodeError::UserNumber);
            }
            pair(&mut out, "FmNumber", &user.get().to_string())?;
            for (k, v) in [
                ("firstname", &p.first_name),
                ("lastname", &p.last_name),
                ("nickname", &p.nickname),
                ("city", &p.city),
                ("familyname", &p.family_name),
                ("familycity", &p.family_city),
            ] {
                pair(&mut out, k, v)?;
            }
            pair(
                &mut out,
                "gender",
                match p.gender {
                    Some(Gender::Female) => "2",
                    Some(Gender::Male) => "1",
                    None => "0",
                },
            )?;
            pair(
                &mut out,
                "birthyear",
                &p.birth_year.unwrap_or(0).to_string(),
            )?;
        }
        ReadResult::Page(page) => {
            if page.people.len() > PAGE_SIZE {
                return Err(DecodeError::Length);
            }
            for person in &page.people {
                person.profile.validate().map_err(|_| DecodeError::Length)?;
                if person.user.get() > 0x00ff_ffff {
                    return Err(DecodeError::UserNumber);
                }
                pair(&mut out, "FmNumber", &person.user.get().to_string())?;
                let p = &person.profile;
                // The GG6 search scanner treats ANY double NUL as a record boundary.
                for (k, v) in [
                    ("firstname", &p.first_name),
                    ("nickname", &p.nickname),
                    ("city", &p.city),
                ] {
                    if !v.is_empty() {
                        pair(&mut out, k, v)?;
                    }
                }
                if let Some(year) = p.birth_year {
                    pair(&mut out, "birthyear", &year.to_string())?;
                }
                pair(&mut out, "FmStatus", &status(&person.presence).to_string())?;
                out.push(0);
            }
            pair(&mut out, "nextstart", &page.next_start.to_string())?;
        }
    }
    if out.len() > 16384 {
        return Err(DecodeError::Length);
    }
    Ok(frame(out))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_read_codes_unset_sequences_and_sparse_search_rows() {
        let request = decode(&[2, 0, 0, 0, 0]).unwrap();
        assert_eq!(request.sequence, 0);
        let own = result(
            0,
            UserId::new(7).unwrap(),
            &ReadResult::Own(Profile::default()),
        )
        .unwrap();
        assert_eq!(&own.payload[..5], &[3, 0, 0, 0, 0]);
        let p = Profile {
            first_name: "Łucja".into(),
            birth_year: Some(1987),
            ..Default::default()
        };
        let page = ReadResult::Page(Page {
            people: vec![Person {
                user: UserId::new(7).unwrap(),
                profile: p,
                presence: Presence::offline(),
            }],
            next_start: 0,
        });
        let out = result(9, UserId::new(8).unwrap(), &page).unwrap();
        assert_eq!(
            out.payload
                .windows(2)
                .skip(5)
                .filter(|b| *b == [0, 0])
                .count(),
            1
        );
        assert!(decode(&[3, 0, 0, 0, 0]).is_err());
        assert!(decode(b"\x01\0\0\0\0FmNumber\0\x37\0").is_err());
        assert!(decode(b"\x01\0\0\0\0nickname\0x\0NICKNAME\0y\0").is_err());
        let mut form = vec![1, 0, 0, 0, 0];
        pair(&mut form, "gender", "2").unwrap();
        let Operation::Write(p) = decode(&form).unwrap().operation else {
            panic!()
        };
        assert!(p.gender == Some(Gender::Female) && p.is_empty());
    }
}
