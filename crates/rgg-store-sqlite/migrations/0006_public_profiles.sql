-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SPDX-FileCopyrightText: 2026 webs1821
CREATE TABLE public_profiles (
    uin INTEGER PRIMARY KEY REFERENCES accounts(uin) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK(typeof(revision)='integer' AND revision>0),
    document TEXT NOT NULL CHECK(length(CAST(document AS BLOB))<=4096),
    first_key TEXT NOT NULL CHECK(length(CAST(first_key AS BLOB))<=180),
    last_key TEXT NOT NULL CHECK(length(CAST(last_key AS BLOB))<=180),
    nick_key TEXT NOT NULL CHECK(length(CAST(nick_key AS BLOB))<=180),
    city_key TEXT NOT NULL CHECK(length(CAST(city_key AS BLOB))<=180),
    family_key TEXT NOT NULL CHECK(length(CAST(family_key AS BLOB))<=180),
    family_city_key TEXT NOT NULL CHECK(length(CAST(family_city_key AS BLOB))<=180),
    gender INTEGER CHECK(gender IN (1,2)),
    birth_year INTEGER CHECK(birth_year BETWEEN 1900 AND 2106)
);
CREATE INDEX profiles_year ON public_profiles(birth_year,uin);
CREATE TABLE directory_rate (
    uin INTEGER PRIMARY KEY REFERENCES accounts(uin) ON DELETE CASCADE,
    window INTEGER NOT NULL CHECK(window>=0),
    used INTEGER NOT NULL CHECK(used BETWEEN 1 AND 20)
);
