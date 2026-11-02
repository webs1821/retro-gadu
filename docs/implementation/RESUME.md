# Punkt wznowienia — 2026-11-02, dev.32

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

## Bieżący stan

Dev.32 / schema14: reset administratora offline z obowiązkową kopią, potwierdzeniem
UIN, oryginalnymi kluczami, atomową zmianą sekretu/wersji/generacji/limitu/audytu.
Dokumentacja: PASSWORD-RESET.md; pełne dowody: password-reset-2026-11-02-dev32.json
i VERIFICATION-2026-11-02-dev32.md. 128 Rust, fmt, Clippy all-features, 13 grup
regresji, migracja dev31 oraz awarie resetu/zmian haseł: PASS. Bez nowych bibliotek.
Przygotowano lokalny commit; sprawdzić git log/status, nie zakładać braku commitu.

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

Przygotować dłuższy, z góry określony test stabilności (soak), najpierw krótko
sprawdzić jego sterownik. Nie nazywać próby debug/krótkiej pełną kwalifikacją R0.
Obecny r0_workload ma limit1200s i polega na świeżej historii; cykle na tej samej
bazie wymagają poprawnego rozliczenia początkowych liczników. Oryginalny GG6,
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
