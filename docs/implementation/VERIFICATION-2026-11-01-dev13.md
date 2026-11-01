# Weryfikacja rGG 0.1.0-dev.13 — 2026-11-01

Linux/WSL, Rust 1.99.1. **71 testów Rust**, fmt i Clippy wszystkich celów/features
bez ostrzeżeń. Nowe testy sprawdzają 32 zachowane klucze, reload, odmowę
nadpisania oraz wycofanie całej partii wraz z nonce przy kolizji drugiego rekordu.

`keys_cli.py` — dwa scenariusze zwykłe i trzy kontrolowane awarie:

- Wszystkie konta, także inactive, otrzymują nowy klucz i świeże nonce;
  credential_version oraz hasła są niezmienne. Kolejna rotacja zachowuje stare
  klucze, nowe konto używa nowego aktywnego klucza. Logowanie GG6 działa.
- Własny keyring kompletnej kopii sprzed rotacji pozwala odtworzyć dokładne
  pierwotne koperty i zalogować się starym hasłem; pliki backupu są niezmienne.
- Brak backupu, aktywny serwer, błędny lub obcy keys.pending i uszkodzone drugie
  konto powodują odmowę bez zastąpienia aktualnego keyringu.
- Process-exit przed publikacją klucza, po publikacji i po pierwszej partii:
  nadal można zalogować wszystkie konta; keys-resume kończy tę samą rotację,
  nie dopisuje kolejnego klucza. Ponowne resume jest no-op.

Przeszły również dwa scenariusze backup/restore z dev.12 i jego dwie awarie,
accounts_cli oraz regresja gg6_sessions. Zwykły końcowy build bez test-hooks;
zwykłe scenariusze rotacji powtórzone na tym buildzie. Dane wyłącznie syntetyczne.

10 modułów, 8 migracji, 190 zewnętrznych wersji zależności, brak nowych bibliotek.
[RustSec](cargo-audit-2026-11-01-dev13.json): 0 podatności, 0 ostrzeżeń.
Cargo.lock SHA-256: `578721cee59069ffdf526f4fc7d351bf55401168873b324e6802e569982009c1`.
Kontrole repo, pochodzenia, GG32, inwentarza i diff poprawne; plan niezmieniony.
Nie testowano zaniku zasilania, natywnego Windows ani klienta GG6. Ograniczenia
w [kontrakcie rotacji](KEY-ROTATION.md). Bez push i wdrożenia.
