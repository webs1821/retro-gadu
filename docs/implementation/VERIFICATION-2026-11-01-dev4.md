# Weryfikacja rGG 0.1.0-dev.4 — 2026-11-01

Wyłącznie lokalnie, Linux/WSL, Rust 1.99.1, syntetyczne konta i bazy tymczasowe.

| Kontrola | Wynik |
|---|---|
| Formatowanie i Clippy, wszystkie cele i funkcje | poprawnie, bez ostrzeżeń |
| Testy Rust | 44 przeszły |
| Cykl życia i konta CLI | oba zestawy przeszły, schemat 5 |
| Sesje i kolejka GG6 | 5 grup TCP, dodatkowo 4 kontrolowane awarie |
| Obecność i kontakty GG6 | 4 grupy TCP, dodatkowo 2 kontrolowane awarie |
| Końcowy zwykły build | wszystkie 4 zestawy procesu/TCP przeszły |
| Pochodzenie i granice modułów | 4 wpisy, 9 modułów, 5 migracji |
| GG32 | 6 przypiętych wektorów |
| Inwentaryzacja / RustSec | 190 wersji zewnętrznych; 0 znanych podatności, 0 ostrzeżeń |

Nowe testy pokrywają statusy i CP1250, opisy 0/1/69/70/71 bajtów, czas powrotu,
różnicę ramek snapshot/zmiana, brak wycieku niewidoczności, blokowanie w obie
strony, tryb tylko znajomi, początkową bramkę Ready, trwały offline po restarcie,
zerwanie sesji, 4000 subskrypcji i odmowę 4001. Test aplikacji trzyma aktywną
publikację i sprawdza, że commit niewidoczności czeka na zwolnienie jej pozwolenia.

Archiwa sprawdzono na granicach 2047 bajtów, z opóźnieniem, równoległym eksportem,
timeoutem, zastąpieniem PUT, zerwaniem i restartem. Kontrolowane awarie przed
commit zachowują starą wersję, a po commit zachowują nową mimo utraty ACK.
Stara sesja i inne konto nie mogą podmienić archiwum. Integralność SQLite poprawna.

Końcowe binaria nie mają `test-hooks`. Nie pozostawiono serwerów próbnych w tle.
Nie wykonano push, wdrożenia ani operacji na produkcji. Oryginalnego GG6 i buildu
Windows nie kwalifikowano. Szczególnie upload zakończony pełnym blokiem bez
terminatora i klienci czekający na ACK każdego fragmentu pozostają ograniczeniem
opisanym w [kontrakcie obecności i kontaktów](PRESENCE-AND-CONTACTS.md).

Nie zmieniono zewnętrznych zależności. [Audyt RustSec](cargo-audit-2026-11-01-dev4.json)
oraz [inwentaryzacja](dependencies.json) zapisują dokładne dane.
SHA-256 Cargo.lock: `31f15fa8725f74ceb3e268c4fbc1a8a99c31972bf1feb80b6ac246dd29f19fbb`.
Plan i recenzje niezmienione. SHA-256 PLAN_IMPLEMENTACJI.md:
`cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec`.
