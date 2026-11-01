# Weryfikacja rGG 0.1.0-dev.9 — 2026-11-01

Linux/WSL, Rust 1.99.1. 58 testów Rust przeszło; formatowanie i Clippy wszystkich
celów/funkcji bez ostrzeżeń. Trzy grupy TCP konferencji: cztery osoby i dokładnie
trzy kopie, porządek uczestników/style po restarcie, prywatność, brak ACK na
żądanie, błędne/duplikowane opcje, granice liczby osób i kwoty bajtów metadanych.
Test repozytorium odrzuca uszkodzony kontekst/tożsamości i usuwa treść terminalną.

Build test-hooks przeszedł dwie awarie konferencji przed/po commit oraz cztery
dotychczasowe awarie tekstu przed/po commit i przed/po zapisie TCP. Po odbudowie
zwykłej binarki ponownie przeszły konferencje i wszystkie regresje: sesje (5),
obecność/kontakty (4), katalog (3), rejestracja (4), statystyki (3), discovery (2),
CLI i cykl życia. Końcowa binarka nie zawiera włączonych hooków awarii.

Migracja end-to-end: poprzedni skompilowany checkpoint dev.8 (`ea92d62`) stworzył
schemat 6, konta i oczekującą wiadomość ze stylem. Dev.9 otworzył te same dane,
zastosował schemat 7 i dostarczył starą wiadomość po prawidłowym logowaniu.
Nowa konferencja na tej bazie także przeszła; integrity_check OK.
Opcja testu: `--baseline-dir /tmp/rgg-baseline-dev8`. Kopie starych binarek:

- server SHA-256: `013e22daca8bce59369c17480c5f4956cecc9e992563f511d9f324bbbd04a69a`
- admin SHA-256: `5f18a910a3d2d35acabd1fca4f7647e7e8d5539b44aff8ec19476399a2000665`

9 modułów, 7 migracji, 4 wpisy pochodzenia. 190 wersji bibliotek bez aktualizacji
zewnętrznych zależności. [RustSec](cargo-audit-2026-11-01-dev9.json): 0 podatności
i 0 ostrzeżeń. Cargo.lock SHA-256: `b32808d92636c1692c38743b8834bfcb5e79e00605a0ad4064c5871ad772c42a`.
Plan/recenzje niezmienione, pochodzenie/GG32/diff poprawne. Testy klienta GG6
natywnego i Windows pozostają otwarte; brak push, wdrożeń i zmian produkcji.
