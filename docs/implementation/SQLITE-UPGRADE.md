# SQLite i SQLx w dev.24

Wbudowany silnik zmieniono z SQLite 3.46.0 na 3.51.3. Cargo.lock przypina
libsqlite3-sys 0.37.0 oraz sqlx-core/sqlx-sqlite 0.9.0. Włączone pozostają tylko
komponenty SQLite, bundled, migracje i Tokio; bez dodatkowych sterowników SQL.
Schema12 i wszystkie pliki migracji SQL pozostały bez zmian.

Powodem jest opisany przez producenta [błąd WAL-reset](https://www.sqlite.org/wal.html#the_wal_reset_bug):
rzadki wyścig zapisu/checkpointu może uszkodzić bazę. Poprawka jest w 3.51.3;
producent podaje również starsze gałęzie z backportem. rGG wymaga co najmniej
3.51.3, z wykluczeniem wycofanej gałęzi3.52, i sprawdza `SELECT sqlite_version()` przy tworzeniu każdego połączenia
magazynu. Nie polega na wersji polecenia sqlite3 ani modułu Python używanego
do niezależnej kontroli wyników.

To oddzielny problem od timeoutu naprawionego w dev.23. Nie zaobserwowano
uszkodzenia danych w tamtych próbach. Jedna kolejka zapisu i trzy czytniki
pozostają; aktualizacja biblioteki nie zastępuje mechanizmu kolejności zapisów.

SQLx0.9 wymaga `SqlStr` dla osadzonych migracji. Zastosowano `SqlSafeStr` dla
statycznych `include_str!`, a pomocniki testowe przyjmują `&'static str`.
Nie użyto obejścia AssertSqlSafe i nie zmieniono treści ani sum migracji.
Klucze i format szyfrowania również pozostały te same.

## Zgodność danych sprawdzona na prawdziwych starych binarkach

`tests/sqlite_upgrade.py` wymaga jawnych starszych binarek; odrzuca podstawienie
identycznego serwera/admina. Dev.23 tworzy syntetyczne konta, zaproszenie,
rezerwację usuniętego UIN, profil, archiwum, blokadę, obrazek i wiadomość offline.
Stary admin tworzy, sprawdza i odtwarza swoją kopię do nowego katalogu. Dopiero
potem dev.24 otwiera odtworzone dane, uwierzytelnia klientów i odbiera wiadomości,
odtwarza profil/kontakty/obrazek, przyjmuje nową wiadomość oraz tworzy własny
backup i restore. Sprawdzono integralność/FK i zachowanie wszystkich 12 sum
migracji. Oryginał i archiwum pozostały niezmienione.

Wynik: `sqlite-upgrade-2026-11-01-dev24.json`, z hashami obu par binarek.
Baseline to commit24e0343, pliki zachowane lokalnie w /tmp/rgg-baseline-dev23.
Dodatkowo zwykła regresja WWW sprawdza rzeczywistą migrację schema11→12
z binarek dev.20, czyli również z poprzedniego SQLx.

## Źródła i powtarzalność

- Oficjalne paczki [sqlx-core0.9.0](https://docs.rs/crate/sqlx-core/0.9.0)
  i [sqlx-sqlite0.9.0](https://docs.rs/crate/sqlx-sqlite/0.9.0).
- [libsqlite3-sys0.37.0](https://docs.rs/crate/libsqlite3-sys/0.37.0): sprawdzono
  nagłówek sqlite3.h z oficjalnej paczki; SQLITE_VERSION=3.51.3,
  SQLITE_VERSION_NUMBER=3051003. SQLx dopuszcza >=0.30.1,<0.38.0.
- [Historia wydań SQLite](https://www.sqlite.org/changes.html): wycofane3.52
  nie jest traktowane jako obsługiwany następca3.51.3. Nowsze3.54.1 nie mieści
  się w wydaniach libsqlite3-sys obsługiwanych przez przypięte SQLx0.9.0;
  nie rozszerzano ręcznie ograniczeń zgodności upstream.
- Cargo.lock oraz dependencies.json zawierają wersje, licencje i sumy pakietów.
  Obraz buduje się --frozen i zachowuje komplet vendored źródeł z licencjami.
- RustSec: cargo-audit-2026-11-01-dev24.json; 0 podatności i 0 ostrzeżeń w bazie
  1395 porad, commit2295466c4e9df26647fd67d8fa1c705ac2fc7f81. To kontrola bazy
  RustSec, a nie dowód braku wszystkich możliwych błędów natywnego silnika/OS.

Nowy obraz i pełny R0 wymagają osobnego raportu. Brak push i wdrożenia.
