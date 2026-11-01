# Punkt wznowienia — 2026-11-01, dev.15

Użytkownik wznowił prace i upoważnił do dalszej implementacji. Nadal wyłącznie
lokalnie: testy i commity jako webs1821, bez push i wdrożenia rGG. Branch master,
origin webs1821/retro-gadu nieużywany. Plan i recenzje pozostają niezmienione.
Nie uruchamiać nowych agentów bez zlecenia delegacji. Brak aktywnego celu/narzędzia goal.

Gotowe lokalnie: konta/zaproszenia (także stare kody), LOGIN60/GG32, sesje,
trwały tekst online/offline, obecność/prywatność, archiwa kontaktów, katalog,
discovery, WWW/rejestracja/liczniki, konferencje, klasyczne obrazki bez trwałego
cache oraz katalog pobierania kontrolowany manifestem. CLI ma pełne kopie,
verify/restore, rotację kluczy ze wznowieniem i importer gg-retro.

Dev.15: schema 10, 10 modułów, 190 wersji zewnętrznych zależności, 74 testy Rust.
Pełny import sztucznego przypiętego gg-retro 0.8.3: wszystkie hasła weryfikowane
i przeszyfrowane świeżym keyringiem, dane w jednej transakcji, receipt z hashami,
marker przed utworzeniem bazy, trzy awarie procesu i retry do nowego katalogu.
Sprawdzono TCP po imporcie, zaproszenia, katalog, kontakty, FIFO/obrazek,
backup/restore, błędne dane/klucze i pełną regresję wcześniejszych funkcji.
Rzeczywista migracja dev.14 schema 9 → 10 i restore starej kopii przeszły.
[Kontrakt importu](IMPORT-GG-RETRO.md), [raport](VERIFICATION-2026-11-01-dev15.md).

Import wymaga jawnego potwierdzenia brakującej historii usuniętych UIN/prywatności
i osobnej listy rezerwacji. Pending TTL jest odnawiany, delivered ma tylko nagłówki,
stare tokeny odrzucane; wszystkie decyzje opisane i raportowane. Nie testowano
importu prawdziwej bazy ani klucza. Pochodzenie: źródło MIS referencyjnie, niezależne
wektory ggv1 z libsodium; żadnego skopiowanego kodu gg-retro/libgadu.

Kolejne luki: upload kontaktów (odroczone ACK, pełny końcowy fragment 2047 B),
patcher/pakiet klienta, domena/profil wdrożeniowy, lokalny kontener i pomiary/soak.
Natywny GG6/Windows nadal niekwalifikowane. Do czasu parytetu nie rozważać podmiany.
[PARITY](PARITY.md) zawiera aktualne rozróżnienie kodu i kwalifikacji.

Repo: D:\Dokumenty\Documents\RetroGadu, poza writable roots — polecenia mutujące
wymagają require_escalated z konkretnym uzasadnieniem. Git author webs1821.
WSL: CARGO_HOME=/tmp/gg-retro-toolchain/cargo,
RUSTUP_HOME=/tmp/gg-retro-toolchain/rustup, CARGO_TARGET_DIR=/tmp/rgg-target,
CC=/tmp/gg-retro-toolchain/gcc/cc; dodać CARGO_HOME/bin do PATH.
Binarki: /tmp/rgg-target/debug/rgg-server i rgg-admin, zwykłe bez test-hooks.
Baselines: /tmp/rgg-baseline-dev8, dev9, dev13 i dev14 (osobne katalogi).
Python Windows 3.13 do tomllib/kontroli repo, WSL 3.8 do TCP/process. W WSL brak rg.
Referencja libgadu: 9eb2aa0eb6ad020fda7c7264538e48c7b78ba602,
D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-reference-libgadu.
gg-retro reference: D:\Dokumenty\Documents\gg-retro, ed9640cc247764754b5e47891053eb07a53f76d5.
