# Punkt wznowienia — 2026-11-02, dev.33

## Upoważnienie i granice

Użytkownik wznowił ciągły rozwój lokalny do przerwania przez niego. Nie pushować,
nie wdrażać na Containers, nie zmieniać działającego gg-retro, zainstalowanego GG,
DNS, hosts ani zaufania systemowego. Wcześniejsze odzyskanie hasła własnego konta
użytkownika jest zakończone: nie powtarzać odczytu ani nie zapisywać hasła w repo.
Brak aktywnego goal i oczekujących pytań. Nie uruchamiać nowych subagentów.

Repo `D:\Dokumenty\Documents\RetroGadu`, master. Commity jako webs1821,
`11637724+webs1821@users.noreply.github.com`. Workspace roboczy:
`D:\Dokumenty\Documents\ChatGPT\Serwery`. Zapisy repo/build/git wymagają
eskalacji. Brak AGENTS.md. Plan niezmienny, SHA256 w raporcie dev.32.

## Bieżący stan dev33

131 Rust/fmt/Clippy all-features/build i10regresji PASS. Runtime Unix odrzuca
publiczne istniejące katalogi przed otwarciem DB/lock oraz symlinki/hardlinki/
FIFO/specjalne pliki SQLite i blokady. Dowód błędu32: UID65534 odczytał
syntetyczny e-mail z DB0644 w katalogu0755; klucza0600 nie mógł odczytać.
Fixture Rust wymagają teraz jawnego0700. Schemat14 bez zmian. Inwentarz190
odświeżony i zgodny; audyt RustSec0/0. DokumentyDATA-DIRECTORY/VERIFICATIONdev33.
Zmiany33 przygotowane do commitu: sprawdzić gitstatus/log i dokończyć.
Debugserver33 `e87990579aa028c969da74f024ab0213bee0e1be7e7aa91b32334638a78d752d`,
debugadmin33 `36bf755635abcb958137612e386da23258b0e49360bd9e6b24eb5084ec149c9d`.
Baseline32 /tmp/rgg-baseline-dev32 zachowany; baseline31 także.

UWAGA: prywatny Docker nadal działa, sesja exec80822, socket
/tmp/rgg-docker-local-20261101/docker32.sock, pidfiledaemon32.pid,
execrootrun32, datarootdata29. Stop dopiero po zakończeniu/posprzątaniu prób:
workspace/rgg-container-stop32.py sprawdza PID/cmdline i brak kontenerów/wolumenów.
Nie zabijać innych usług, nie usuwać cache/obrazów/archiwów.

Dev32 źródło59d0f8a12690d49936e6384da1d94c176722c38a ma gotowe obrazy:
runtime sha256:1e882cff0c1f94bbbb19a3a39421a71e93627e3ca995945a2776ce2039127761,
verification sha256:8f9f72b6fdf227449fb3e593bb6e0ef25c458d9d9539755e6663e6e036c2afd3.
33releasegrupy147.358s/runtime/krótkie10s×1 i10s×2 PASS, cleanupPASS. Raporty
wrepo i/tmp/rgg-docker-local-20261101/*dev32-final*. Wcześniejsze eb94baa
obrazy niekwalifikowane (staryhashinwentarza); zachowane. Audyt32 /tmp/rgg-audit32.json.
Pełny godzinny soak32 NIE startował. Przenieść go na poprawiony obraz33.
Sterownik iSOAK.md zapisane w eb94baa, potwierdzone krótkimi próbami.

Następny krok: lokalnycommit33, build runtime/verification33 na istniejącym
prywatnymDocker32, suite34 i runtime, następnie900s×4. Można wykorzystać
workspace rgg-container-*-32-final.py jako wzór; użyć świeżych nazw raportów
i nowego sourcecommit. Krótkie bramki sterownika już potwierdzone, ponawiać
tylko gdy zmieni się sterownik. Podczas długiej próby nie kończyć pracy;
komunikować postęp, reagować na przerwanie użytkownika. Po sukcesie raporty,
archiwumobrazu, cleanup, stopDocker i lokalnycommit. Nadal zero push/deploy.

## Poprzedni stan dev32

Dev.32 / schema14: reset administratora offline z obowiązkową kopią, potwierdzeniem
UIN, oryginalnymi kluczami, atomową zmianą sekretu/wersji/generacji/limitu/audytu.
Dokumentacja: PASSWORD-RESET.md; pełne dowody: password-reset-2026-11-02-dev32.json
i VERIFICATION-2026-11-02-dev32.md. 128 Rust, fmt, Clippy all-features, 13 grup
regresji, migracja dev31 oraz awarie resetu/zmian haseł: PASS. Bez nowych bibliotek.
Lokalny commit resetu: `1ea6e30`.

Ostatni debug32, bez test-hooks, `/tmp/rgg-target/debug`:
- rgg-server: `e5fa5b389e9ea1f6060161a3854663f057e63809e8681b7078cce2e7a3e3ac84`
- rgg-admin: `c3f7025072661e2dd73c05ffd345c9c289bf5fb0dbec939823e51369f8c1a7c9`

Dev31 zachowany w `/tmp/rgg-baseline-dev31`, serwer
`f1176e600dbf8c9e64236341ca7d79014e181a0ec1acb8b1921af28aaafe670f`, admin
`994b02b251bea7d208b04e061c6d9ae79cb076da44208d87f42ba7d5a7b77e2f`.
Inne starsze baseline pozostają. `/tmp/rgg-target/release` jest stare, nie używać
jako bieżącej wersji. Wszystkie procesy testów dev32 zakończone.

## Zakończone poprzednie kroki

- `745e5d9`: libgadu obrazy/konferencje; `1cb3e0e`: katalog/archiwum kontaktów.
- `3294590`: opisy GG6 bez NUL, potwierdzone błędem dev30 i niezależną libgadu1.7.0.
- `f0aff5f`: dokumentacja obrazu/R0 dev30; `94730ef`: źródła verification w RO /src.
- `90aed8d`: profil HTTPS proxy; `b412748`: metryki obrazów; `16373ff`: advertised GG.

Libgadu: zewnętrzna LGPL2.1 biblioteka tylko do testów, źródła poza repo/runtime.
Własny MIT probe i builder w validation/libgadu_170. Archiwum1.7.0 SHA256
`13b0c4c1471f40022db3fd7d141d5d0b42dd67f0a9ca82db877c39b60e14071a`.
38 plików upstream niezmienionych. Build w `/tmp/rgg-libgadu170-final`, najnowszy
probe `/tmp/rgg-libgadu170-media`, SHA256
`3372c0c8b060ac5e909651f51a6c66f10008e437948f674ac5b556c98e736c99`.
Core, directory i media PASS; JSON i ograniczenia w LIBGADU-COMPATIBILITY.md.
Nie jest to kwalifikacja oryginalnego GUI GG6.

Ostatni kwalifikowany kontener/R0: dev30, źródło94730ef, 32 zestawy release,
600 s / 100 Ready / 6000 wiadomości / 1CPU / 256MiB / swap0, wszystkie bramki PASS.
Archiwum w workspace/rgg-container-2026-11-02-dev30/rgg-dev30-runtime.tar.gz,
SHA256 `2fd69c37b1c65cb9024ff5f131e45d7f91b59e04f3293a4367c22488b42fc2b7`.
Daemon Docker30 zatrzymany, cache w /tmp/rgg-docker-local-20261101 zachowany.
Dokładne ID obrazów, raporty i cgroup: VERIFICATION-2026-11-02-r0-dev30.md.
Obraz30 nie zawiera zmian31/32. Kontenerowe źródła testów muszą być w RO /src;
rozpakowanie ich do /tmp32MiB wcześniej powodowało ENOSPC.

## Następny krok

Sterownik soak przygotowany: tests/r0_workload.py (run_cycles), r0_smoke.py
i container_load.py (--cycles1..4). Progi przed próbą w SOAK.md. Krótka próba
dwóch cykli5s/6sesji/20wiadomości PASS w27,064s; repo JSON. Pełny soak
i kontenerowe bramki jeszcze niewykonane. Następny krok: commit sterownika,
zbudowanie obrazu dev32, suite33grupy + runtime, krótka próba kontenera
(10s×2), następnie pełne900s×4 z progami RSS/FD/wątków. Nie nazywać próby debug/krótkiej pełną kwalifikacją R0.
r0_workload rozlicza początkową historię i unikalną bazę sekwencji kolejnych
cykli, poza cyklami nadal wymaga świeżej historii. Limit1200s/cykl,4cykle. Oryginalny GG6,
długi soak, pełny przegląd bezpieczeństwa i kwalifikacja wdrożeniowa nadal otwarte.
Nie podejmować wdrożenia na podstawie samego wyniku testów syntetycznych.

## Narzędzia

WSL webs(uid1000): Rust1.99.1, CARGO_HOME=/tmp/gg-retro-toolchain/cargo,
RUSTUP_HOME=/tmp/gg-retro-toolchain/rustup, CARGO_TARGET_DIR=/tmp/rgg-target,
CC=/tmp/gg-retro-toolchain/gcc/cc. Dodać $CARGO_HOME/bin do PATH.
Python Windows3.13 do tomllib/provenance; system WSL Python3.10 ma starsze SQLite
bez unixepoch(), w fixture używać CAST(strftime('%s','now') AS INTEGER).
WSL nie ma rg. Izolacja: unshare -Urn, w środku tylko `ip link set lo up`.
Porównywać own /proc/self/ns/net z wartością launchera i socket.if_nameindex(),
nie /sys/class/net ani /proc/1/ns/net. Nie zmieniać sieci hosta.
Raporty dev32: /tmp/rgg-dev32-check. Wrappers workspace rgg-dev32-*.py.
Nie nadpisywać istniejących raportów; zachować próby błędne i finalne.
