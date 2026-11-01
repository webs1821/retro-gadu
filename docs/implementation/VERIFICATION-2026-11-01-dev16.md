# Weryfikacja rGG 0.1.0-dev.16 — 2026-11-01

Linux/WSL, Rust 1.99.1. **77 testów Rust**, fmt i Clippy wszystkich celów/features
przeszły bez ostrzeżeń. 10 modułów, 190 wersji zależności; subtle już był w lock.
Schemat 11, nowe admin_audit; żadnej wcześniejszej migracji nie zmieniono.

Trzy nowe testy store: pełna autoryzacja, odrzucenie identycznego hasła, rollback
kolizji nonce, stara wersja nie zakłada sesji; odmowa aktywnego/wyłączonego konta
i przepełnienia wersji; limit 10000 wpisów, 100 w odczycie i wykluczenie/pruning
starych zdarzeń. Nie ma endpointu zmiany online ani resetu bez starego hasła.

password_cli.py: rzeczywiste CLI i TCP, nowe hasło działa, stare odmawia,
niezmieniony keyring/pozostałe dane i zachowana kolejka obrazka; katalog/kontakty,
kopie sprzed/po zmiany działają z odpowiednimi hasłami. Audyt nie ujawnia sekretów
i zachowuje zdarzenie po retirement. Odmowy dla błędnego hasła/JSON/rozmiaru,
braku konta/klucza, niedokończonej rotacji i działającego serwera. Pomocnik nie
przyjmuje nieinteraktywnego stdin z potencjalnie widocznym wpisywaniem.

Dwa exit(86) przed/po commit: stare/nowe hasło, wersja, nonce i audyt zgodne.
Testowany importer nadal przechodzi normalny roundtrip i trzy swoje awarie.
Końcowy build bez test-hooks; powtórzona normalna zmiana i migracja/restore.

Rzeczywiste binarki dev.15 utworzyły schemat 10, konta, katalog, kontakty i
oczekujący obrazek. Migracja do 11, zmiana hasła i dostarczenie obrazka przeszły.
Osobno restore starej kopii przez dev.15 i zmiana hasła po migracji przez dev.16.
SHA-256 baseline server: `4205a2dbfb6640cbef0cf58b6f9a2e9b9c73771113d3f2b6bf6f05551c399236`,
admin: `758346f1fc2b0a3c5417e703aeefc16c7e234a5fd3cbfcb8e970cbe8a03dc750`.

Regresja: lifecycle, accounts_cli, gg6_sessions, presence_contacts, directory,
conferences, images, http_registration, legacy_invitations, backups_cli, keys_cli.
Pierwsze uruchomienie nowego testu ujawniło zbyt szeroką zamianę oczekiwanego
numeru schematu w istniejących testach; poprawiono wyłącznie właściwe asercje
migracji i ponowiono scenariusze. Serwerowy numer pakietu pozostał poprawny.

[RustSec](cargo-audit-2026-11-01-dev16.json): 0 podatności i 0 ostrzeżeń.
Cargo.lock SHA-256: `ca52c9a3e2dba4cd72b45db7df556775aa61a12fe7182e0405d6a1ca6556ea64`. Plan niezmieniony.
Nie kwalifikowano klienta natywnego ani Windows. Nie czytano prawdziwej bazy
w ramach tych testów, nie wykonywano push ani wdrożenia rGG.
