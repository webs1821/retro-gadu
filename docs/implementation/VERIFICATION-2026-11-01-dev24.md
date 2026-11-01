# Dev.24 — SQLite 3.51.3 i SQLx 0.9.0

Wersja0.1.0-dev.24, schema12, Rust1.99.1, Linux/WSL. Szczegóły zmiany i źródła:
[SQLITE-UPGRADE.md](SQLITE-UPGRADE.md).

- 102 testy Rust, fmt i Clippy all-targets/all-features -D warnings.
- 23 grupy regresji TCP/HTTP/CLI: sqlite-regression-2026-11-01-dev24.json.
- 9 grup kontrolowanych awarii: sqlite-faults-2026-11-01-dev24.json.
  Po próbach przywrócono zwykłe binarki bez test-hooks.
- Zgodność prawdziwego backupu dev.23, danych i nowych zapisów:
  sqlite-upgrade-2026-11-01-dev24.json. Zachowane12 sum migracji, stare hasła,
  offline, obrazek, profil i kontakty; nowe backup/restore przeszło.
- Zwykła regresja hasła WWW obejmuje również binarki dev.20 i migrację11→12.
- 190 wersji zewnętrznych bibliotek, inwentarz zgodny z Cargo.lock; RustSec0/0,
  pełny raport cargo-audit-2026-11-01-dev24.json.
- Oryginalny plan bez zmian: SHA256
  cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec.

Testy nie są dowodem kwalifikacji nowego obrazu, długiego obciążenia ani natywnego
klienta. Nowy pełny R0 i obraz wymagają osobnego raportu z dokładnymi hashami.
Nie wykonano push ani wdrożenia; działająca usługa gg-retro pozostała bez zmian.
