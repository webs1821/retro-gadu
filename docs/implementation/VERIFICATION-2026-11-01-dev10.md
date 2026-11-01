# Weryfikacja rGG 0.1.0-dev.10 — 2026-11-01

Linux/WSL, Rust 1.99.1. **68 testów Rust** przeszło; Clippy wszystkich celów
i funkcji bez ostrzeżeń, formatowanie poprawne. Nowe testy obejmują uprawnienia,
quota zachowanych ofert, zależność publikacji od blokad, CRC i granice pamięci,
liczbę części, kolejność, timeout, anulowanie i granice parserów.

Pięć grup rzeczywistego TCP w `tests/gg6_images.py` przeszło. Obejmują:

- obraz online, pełne CRC przed publikacją, tylko właściwy odbiorca, NO_ACK;
- ofertę offline, brak nadawcy, restart i pobranie po usunięciu treści;
- niewidocznego nadawcę respektującego wcześniej udzieloną ofertę;
- złe CRC/kolejność, blokadę w połowie, wymianę sesji i błędne pakiety;
- LOGIN60, prywatność `image_size`, 0/1 KiB oraz granicę 255 KiB;
- konferencję, która nie nadaje dostępu innym wymienionym uczestnikom.

Build test-hooks: 2 awarie obrazu przed/po commit potwierdzają wspólną transakcję
wiadomości/oferty; 4 dotychczasowe awarie kolejki tekstu, 2 kontaktów i 2 konferencji
także przeszły. Zwykła binarka odbudowana po testach i ponownie sprawdzona przez
pełny zestaw obrazków oraz lifecycle. Końcowy build **bez test-hooks**.

Migracja rzeczywistego checkpointu dev.9: stare binarki tworzą schemat 7 i wiadomość
offline, nowe stosują schemat 8, dostarczają starą wiadomość, następnie obsługują
nowy obraz. Kopie `/tmp/rgg-baseline-dev9`:

- server SHA-256: `7f88a29826eb5ddd219e03cc33793bf797287ce80a4c4698cfb0a08f8914d34d`
- admin SHA-256: `4e47d8f57c0072f8bcd891d41b170d83acde6d16cecb7edb1cc1b9826555e191`

Przeszła także migracja z rzeczywistego dev.8 (schemat 6) do bieżącego schematu 8,
z zachowaniem stylów/konferencji. Regresja: sesje, obecność/kontakty, konferencje,
katalog, discovery, rejestracja, statystyki, CLI, lifecycle. W teście lifecycle
zaktualizowano oczekiwany numer schematu 7 → 8; końcowy test przeszedł.

10 modułów, 8 migracji, 4 wpisy pochodzenia, 190 zewnętrznych wersji zależności.
`crc` 3.4.0 był już pośrednią zależnością; teraz używa go także `rgg-transfers`.
[RustSec](cargo-audit-2026-11-01-dev10.json): 0 podatności, 0 ostrzeżeń.
Cargo.lock SHA-256: `ab1937249e6e5e70ea56c6adaac8bbea8effaa7b9a2a00f58a93ed855f5a12ee`.
Plan ma nadal SHA-256 `cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec`;
recenzje bez zmian. Kontrole pochodzenia, GG32, dependency inventory i diff poprawne.

Nie kwalifikowano natywnego GG6/Windows, zachowania UI ani zużycia RSS w soak test.
Brak push i wdrożenia rGG. Testy używały wyłącznie lokalnych, sztucznych danych.
