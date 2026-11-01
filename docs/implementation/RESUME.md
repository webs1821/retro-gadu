# Punkt wznowienia — 2026-11-01, dev.11

Użytkownik wznowił prace. Nadal wyłącznie lokalnie: testy i commity jako webs1821,
bez push i wdrożenia rGG. Master, origin wskazuje webs1821/retro-gadu; nie był używany.
Plan implementacji i recenzje pozostają niezmienione.

Gotowe: konta/zaproszenia, logowanie, sesje, kolejka offline, obecność/prywatność,
archiwa kontaktów, katalog, discovery, WWW/rejestracja/liczniki, konferencje,
obrazki klasyczne bez trwałego cache (commit 11cf7a1) i opcjonalny katalog pobierania
z manifestem, hashami, pochodzeniem oraz instrukcjami. Schemat 8, wersja 0.1.0-dev.11.

[Kontrakt obrazków](IMAGES.md) i [weryfikacja](VERIFICATION-2026-11-01-dev10.md):
68 testów Rust, TCP/process/CLI/HTTP, awarie i rzeczywiste migracje dev.8/dev.9.
10 modułów; nowy rgg-transfers zależy od application/domain, nigdy odwrotnie.
190 wersji zależności, CRC z już obecnego crc 3.4.0. Zwykły końcowy build bez hooków.
Natywny GG6 i Windows nadal niekwalifikowane.

Dev.11: 69 testów Rust, 4 grupy HTTP pobierania, regresje WWW/GG i podgląd desktop;
[raport](VERIFICATION-2026-11-01-dev11.md). Zwykłe binarki, podgląd zamknięty.
Katalog pobierania gotowy; prawdziwe pakiety klienta nadal nie są dołączone.

Następne luki do parytetu: patcher/pakiet klienta, konfiguracja docelowej domeny, admin/backup/restore,
importer dotychczasowego gg-retro, lokalny kontener, pomiary i testy natywne.
Nie kopiować binarek klienta do kodu AGPL ani kodu gg-retro (niestandardowa MIS).
Szczegóły [PARITY](PARITY.md) oraz plan. Przy obsłudze pobierania użyć jawnego
manifestu, limitów plików, braku ścieżek z żądań i istniejącej ochrony Host/IP WWW.

Repo: D:\Dokumenty\Documents\RetroGadu.
WSL: CARGO_HOME=/tmp/gg-retro-toolchain/cargo,
RUSTUP_HOME=/tmp/gg-retro-toolchain/rustup, CARGO_TARGET_DIR=/tmp/rgg-target,
CC=/tmp/gg-retro-toolchain/gcc/cc; dodać CARGO_HOME/bin do PATH.
Binarki: /tmp/rgg-target/debug/rgg-server oraz rgg-admin.
Zachowane stare binarki do testów: /tmp/rgg-baseline-dev8 i /tmp/rgg-baseline-dev9.
Python Windows 3.13 do tomllib i kontroli repo; WSL Python 3.8 do TCP/process.
W WSL brak rg. Oficjalne źródła libgadu: przypięte 9eb2aa0eb6ad020fda7c7264538e48c7b78ba602,
lokalne referencje w D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-reference-libgadu.
