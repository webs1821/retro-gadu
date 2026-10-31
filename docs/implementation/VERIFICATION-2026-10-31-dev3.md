# Weryfikacja rGG 0.1.0-dev.3 — 2026-10-31

Implementacja: commit `93f8fa8`. Środowisko Linux/WSL, Rust 1.99.1,
SQLite bundled przez sqlx-sqlite 0.8.6. Wszystkie procesy, konta, hasła,
wiadomości i bazy testowe były lokalne i syntetyczne.

| Kontrola | Wynik |
|---|---|
| `cargo fmt --all -- --check` | poprawnie |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | bez ostrzeżeń |
| `cargo test --workspace --locked` | 37 testów Rust przeszło |
| `tests/lifecycle.py` | live/ready, lock, SIGKILL/restart, nowa epoka, schemat 4, SIGTERM, public bind refusal |
| `tests/accounts_cli.py` | istniejące scenariusze rejestracji, szyfrowania i blokady CLI/serwer przeszły |
| `tests/gg6_sessions.py` — zwykły build | pięć grup testów TCP przeszło |
| `tests/gg6_sessions.py --faults` — build test-hooks | pięć grup TCP i cztery granice awarii przeszły |
| `tools/verify_repository.py` | 3 wpisy pochodzenia, 9 modułów, 4 migracje |
| `tools/gg32_vectors.py` | 6 przypiętych wektorów zgodnych |
| Inwentaryzacja zależności | 190 zewnętrznych wersji, komplet metadanych licencji |
| RustSec / cargo-audit 0.22.2 | 0 znanych podatności, 0 ostrzeżeń |

Testy obejmują błędne hasło, nieznany numer, powtórzoną odpowiedź na inne wyzwanie,
timeout logowania, komendę przed logowaniem i przed Ready, drugie logowanie,
barierę zamknięcia poprzedniego writera, stare poświadczenia i stary detach.
Sprawdzają CP1250 i style, FIFO mimo cofnięcia zegara i ponownego użycia seq,
brak ACK przy fladze 0x20, widoczny commit przed ACK, odtworzenie kolejki,
pominięte wakeup, blokady po restarcie i niepełnej synchronizacji, brak
wskrzeszenia po odblokowaniu, limity połączeń i wiadomości, 12 równoległych
zapisów do limitu trzech, wolnego odbiorcę, expiry, stare tokeny lease,
retencję treści, limit prób, wadliwy payload oraz ograniczenia SQLite z NULL.

Zestaw awarii uruchamia testowy serwer z `test-hooks` i kończy proces w czterech
punktach. Przed commit nie pozostaje wiadomość. Po commit przed ACK wiadomość
pozostaje w kolejce. Po claim i po pełnym zapisie przed utrwaleniem wyniku
następuje ponowienie po restarcie. Ostatni przypadek potwierdza dopuszczalny
duplikat na granicy awarii, nie gwarancję exactly-once.

Po testach awarii ponownie zbudowano zwykłe binaria i wykonano wszystkie trzy
zestawy procesu/TCP bez funkcji testowej. Nie pozostawiono testowego serwera
działającego w tle. Workflow CI rozszerzono lokalnie; nie uruchamiano GitHub Actions.

## Zależności i niezmienność planu

[Raport audytu](cargo-audit-2026-10-31-dev3.json): baza RustSec commit
`2295466c4e9df26647fd67d8fa1c705ac2fc7f81`, aktualizacja 2026-10-30,
1395 zgłoszeń. Lockfile obejmuje 199 pakietów, w tym 9 modułów rGG.
SHA-256 `Cargo.lock`:
`525c320dbc8ef5307d029c7239afb303d164a51c30c78a4538a78917f72b7f86`.
Nowa bezpośrednia zależność `subtle` służy porównaniu odpowiedzi logowania.
`socket2` był już obecny przechodnio; teraz ogranicza też bufor wysyłania socketu.

Plan i recenzje pozostają bez zmian. SHA-256 `PLAN_IMPLEMENTACJI.md`:
`cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec`.
Poprzedni [raport dev.2](VERIFICATION-2026-10-31.md) jest zapisem historycznym.

## Granice potwierdzenia

Klient Python koduje pakiety niezależnie od adaptera Rust, ale używa istniejącego,
jawnie przypisanego portu GG32 do obliczenia odpowiedzi. Nie zastępuje testu
oryginalnych GG 6.0/6.1, ich interfejsu ACK ani automatycznego reconnectu.
Nie kwalifikowano natywnego buildu Windows, długiego obciążenia/soak, migracji
produkcyjnej bazy, rotacji kluczy ani backup/restore. Pełna obecność, katalog,
obrazki, konferencje, kontakty na serwerze i WWW pozostają w macierzy braków.
Znane ograniczenia dostarczania starego protokołu opisuje
[dokument sesji i kolejki](SESSIONS-AND-DELIVERY.md).

Nie utworzono zdalnego repozytorium, nie wykonano push ani wdrożenia na Containers.
