# Weryfikacja rGG 0.1.0-dev.14 — 2026-11-01

Linux/WSL, Rust 1.99.1. **72 testy Rust**, fmt i Clippy wszystkich celów/features
bez ostrzeżeń. Schemat 9, nowa pusta tabela legacy_invitations; żadna wcześniej
wydana migracja nie została zmieniona. Liczba modułów 10, zależności 190.

`legacy_invitations.py`: case-sensitive hashe starego formatu, rollback zużycia
po kolizji numeru, wyłączenie kodu, wyczerpanie i brak odzyskania użyć po restore.
Przez HTTP testowano 128 znaków i 12 konkurencyjnych żądań: dokładnie jedna
rejestracja z jednym pozostałym użyciem. Po rejestracji działa logowanie GG6.
Wyczerpany kod rGG o identycznym tekście jak stary kod nie zużywa starej puli.

Rzeczywiste binarki dev.13 utworzyły schemat 8, konta, profil/kontakty, trwałą
kolejkę i oferty obrazka. Migracja do dev.14 zachowała konta, liczniki i rezerwację
UIN, a odbiorca dostał wiadomość i obrazek. Nowy verifier odmówił kopii schematu 8;
restore narzędziem dev.13 do świeżego katalogu i migracja tego katalogu do 9 przeszły.
SHA-256 zachowanych binarek odniesienia:

- admin: `f4791766509bfc56c6921ac0c3a777cf3c4847375d8fbb856d2c046707fb9058`
- server: `b4fa87239aa649921c6bb1fce9de0b89cb1e872a40c32d29bbde8504186ff264`

Przeszły 4 grupy zwykłej rejestracji HTTP, backup/restore, rotacja kluczy,
lifecycle i 5 grup obrazków, także rzeczywista migracja danych dev.9 → dev.14.
Po końcowej definicji migracji powtórzono testy store (33) i nowe scenariusze
migracji/zaproszeń. Nie powtarzano niezmienionych hooków awarii rotacji i backupu.
Końcowe binarki są zwykłym buildem bez test-hooks. Nie kwalifikowano natywnego GG6.

[RustSec](cargo-audit-2026-11-01-dev14.json): 0 podatności, 0 ostrzeżeń.
Cargo.lock SHA-256: `f8f7fcbf15f9aa2782b148628b9c3325b5ec8879df02c3ad83f6fda686c66cfd`.
Inwentarz, provenance, kontrola repo i diff poprawne. Plan niezmieniony.
Wyłącznie dane syntetyczne, bez push, odczytu produkcyjnej bazy ani wdrożenia.
Właściwy importer całego forka nadal do implementacji.
