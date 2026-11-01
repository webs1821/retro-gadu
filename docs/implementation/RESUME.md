# Punkt wznowienia — 2026-11-01, dev.23

## Zakres i najbliższa praca

Użytkownik wznowił ciągły rozwój lokalny do przerwania przez niego. Nie pushować,
nie wdrażać na Containers, nie zmieniać działającego gg-retro, DNS ani hosts.
Doraźne odzyskanie własnego hasła użytkownika wykonano wcześniej; nie powtarzać
odczytu i nie zapisywać hasła w repo. Nie ma aktywnego goal ani nowych subagentów.

Dev.23: naprawiono timeout przy odtwarzaniu dużej kolejki. SqliteStore ma teraz
writer max1 i pool max3 otwierany read_only=true/query_only=ON. Wszystkie zapisy,
transakcje modyfikujące i VACUUM INTO idą przez writer. Snapshoty metryk/statystyk
pozostają transakcjami odczytowymi. Inicjalizacja przed czytnikami, close obu pul.
Nadal WAL/FULL, schema12, te same terminy. 101 testów Rust, fmt, Clippy,
23 grupy TCP/HTTP/CLI i9 grup --faults przeszły. Binarki debug przywrócone normalne.
Nowy tests/gg6_concurrent_replay.py jest w CI/container_suite: syntetyczna historia
3000 + kolejka3000,20 klientów, payload/FIFO/integrity. Czas debug około48s;
nie jest testem pojemności. Surowe wyniki sqlite-writer-*.json.

Kolejność dalszej pracy:
1. Zakończyć lokalny commit dev.23, jeżeli jeszcze nie istnieje.
2. Aktualizacja SQLx0.8.6 / libsqlite3-sys0.30.1 (wbudowane SQLite3.46.0).
   Producent opisuje WAL-reset bug, poprawiony od3.51.3, także3.44.6/3.50.7.
   https://www.sqlite.org/wal.html#the_wal_reset_bug
   SQLx0.9.0 jest stabilny, dopuszcza libsqlite3-sys >=0.30.1,<0.38.0.
   libsqlite3-sys0.37.0 opublikowany2026-03-15; sprawdzić faktyczny nagłówek
   i SELECT sqlite_version() po pobraniu. Nie kopiować ręcznie amalgamacji do repo.
   Przypiąć zgodne wersje, zaktualizować inwentarz, przejść testy i RustSec.
   Zachować stare binarki do migracji/restore. To oddzielna zmiana od writer FIFO.
3. Nowy obraz z zatwierdzonego commitu, suite26, runtime restart/restore i R0
   600s/6000 z niezmienionymi progami. Dopiero po pełnym sukcesie eksport artefaktu.
4. Wrócić do poprawki końca uploadu GG6 w generatorze klienta (opis niżej).

## Wyniki obrazu dev.22 i odtworzenie błędu

Źródło a5d55d37ba29d00b7772629b9d6ce99452a0f1a9, wersja0.1.0-dev.22.
Runtime sha256:8bb71c2bfada6511553d0989514de515530884f4c351645fbb49b9cc68214632.
Verification sha256:243c59130e1538d6c8b2aa50b47835f9fa1392cf49c76593a98254cfea8696ea.
Server release bfa935ea649efddad5e8459ec6c6b2eae09cb17971f91e0fd6b6c3521870c3ae.
Source archive6704391cb029abdc801195c6d1614e2c675237eaf28ed7f9559f0bab7ba07e47.
25 grup release przeszło69.51s, restart/wolumen/backup-restore także. Swap w tych
kontrolach faktycznie max; nie udawać0. Dłuższy R0 osobno ustawił i potwierdził
swap0. Nadano6000 przez600.000s/100Ready, ale końcowy replay3000 zakończył EOF.
Ta próba NIE przeszła. Zasoby zachowane w surowym raporcie, cleanup=true.
Średni CPU fazy12.299%, końcowe60s14.206%, RSS peak20824064B; bez OOM.

Diagnostyka przy1CPU/256MiB/swap0:
- samo3000 offline: sukces15.519s;
- 3000 terminalnych+3000 offline: EOF, delivery timeout, odbiorca118;
- writer FIFO, kandydat release: sukces22.516s,6000 TWU,0sesji/payloadów,FK/DB ok.
SHA kandydata cd3dafadcfbd373f1c2e8e98339287793661db592b8ac224819f345d39f047ce.
Kandydat miał jeszcze label dev22; diagnostyka nie kwalifikuje nowego obrazu.
Python verifier SQLite3.40.1 NIE jest wersją SQLite wbudowaną w rGG.
Próba debug nie zakończyła seedowania online przy100/s; driver3 ma jawne
bariery Ready/ukończenia historii. Replay nadal30s. Zachowane także wcześniejsze
nieukończone przygotowanie i start przed ukończeniem builda (brak uruchomienia).

Raporty w docs/implementation i kopia:
D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-container-2026-11-01-dev22.
Nie utworzono kwalifikowanego tar.gz dev22. Ostatnia udana krótka próbka R0
oraz archiwum to nadal dev19, udokumentowane w VERIFICATION-2026-11-01-r0.md.
Nie usuwać starych archiwów dev18/dev19 ani raportów niepowodzeń.

## Silnik testowy i narzędzia

Prywatny Docker29.9.2/runc1.5.2 w WSL:
/tmp/rgg-docker-local-20261101 (root0700), bin/, data29/, run29/,
daemon29.json, docker29.sock, daemon29.pid, daemon-dev22.log.
Daemon działa w sesji71330; po zakończeniu prób lub pauzie zatrzymać tylko ten
sprawdzony PID (SIGTERM), wcześniej sprawdzić brak własnych kontenerów/wolumenów.
Docker Desktop pozostaje wyłączony. Zawsze jawny --host unix://... i pusty
osobny --config, bez DOCKER_*; nie używać domyślnego kontekstu.
Daemon bez bridge/iptables/forwardingu. Testy --network none, klienci współdzielą
wyłącznie netns serwera. R0 cgroup helper zmienia swap tylko po sprawdzeniu
własnego kontenera/UID/limitów/etykiety. Nigdy globalna zmiana cgroup.

Workspace repo D:\Dokumenty\Documents\RetroGadu, master. Zapis/build/git wymagają
require_escalated (poza writable roots). Git author webs1821,
11637724+webs1821@users.noreply.github.com. Nie było push.
Python Windows C:\Program Files\Python313\python.exe, Node C:\Program Files\nodejs\node.exe.
WSL python3 jest3.10; tools z tomllib uruchamiać Python3.13 Windows lub image3.13.
WSL cargo: CARGO_HOME=/tmp/gg-retro-toolchain/cargo,
RUSTUP_HOME=/tmp/gg-retro-toolchain/rustup, CARGO_TARGET_DIR=/tmp/rgg-target,
CC=/tmp/gg-retro-toolchain/gcc/cc; PATH=$CARGO_HOME/bin:$PATH. Rust1.99.1.
Normalne debug binarki /tmp/rgg-target/debug; release w /tmp/rgg-target/release
zawiera kandydata writer jeszcze sprzed bumpa dev23. Baselines dev8/dev9/dev13/
dev14/dev15/dev20 oraz /tmp/rgg-baseline-dev22 (normalne binarki sprzed poprawki).

## Analiza końca listy kontaktów — jeszcze bez poprawki

Workspace D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-client-analysis-2026-11-01:
gg-original-analysis.bin (nie uruchomiono jako aplikacji), disassembly.txt,
emulate_contacts.py, original-emulation.json, deps/Unicorn2.1.4. W tym samym
katalogu tymczasowe replay_diagnostic1/2/3.py i diagnose_container_replay.py.
Runner może podstawić --candidate binarkę do własnego wolumenu; taka diagnostyka
z bazowym obrazem NIE kwalifikuje identyczności źródła/obrazu.

Oryginał GG6.1 SHA74b78af318c52f86dff8a7ee27a7a0b5ea545167aebb594a8a559ec97f2e3a57,
wydobyty z przypiętego gg61.exe przez własny parser. Funkcja uploadu VA4249ef–424a89:
bloki max2047, limit całości50000, pierwszy typ0, dalsze1, packet0x16. Wszystkie
wywołania send w pętli, licznik potwierdzeń this+0x9c zwiększany dla każdego.
Dla dodatniej wielokrotności2047 NIE wysyła pustego MORE. Pusta lista wysyła PUT0.
Null pointer nie wysyła nic. Potwierdzone13 przypadkami w emulatorze CPU, który
przechwytuje strlen/memcpy/send; zero WindowsAPI/sieci i żadnego startu aplikacji.
Stan zachowania całego GG/native pozostaje NIEkwalifikowany.

Możliwa poprawka: własny stub w zerowym paddingu .text VA479400/raw79400,
rozszerzyć VirtualSize sekcji (obecnie78398; raw79000). Hook predicate od424a39
(do424a42,9B) do stuba. Powtórzyć decyzję normalnego send, a na końcu dodać
pusty MORE tylko gdy ebx>0 i ebx%2047==0; zwiększyć licznik ACK i od razu wyjść.
Nie dopisywać pustego MORE po krótkim fragmencie (serwer już opublikował listę).
Nie zgadywać końca po timeout ani osłabiać atomowości serwera. Zweryfikować
instrukcje/stos/registers w emulatorze, PE/Windows resource reader, Python/JS
pełnym porównaniem, potem protokołem TCP. Aktualny generator/mapa jeszcze bez zmian.

Unicorn wheel win64 hash d7107500c64ce5c168fbff6bef9485b5db1350050036f4cea568650cf8bdbdf5.
To narzędzie testowe poza runtime. Emulator napisany niezależnie jako osobny
program MIT, bez importowania kodu AGPL patchera; nie kopiować zamkniętego klienta
ani pełnego disassembly do Git. Recepta patchera zachowuje pochodzenie w THIRD_PARTY.

## Pozostałe odniesienia

SQLITE-WRITER.md i VERIFICATION-2026-11-01-dev23.md — najnowsze dowody.
PARITY.md rozróżnia implementację od kwalifikacji; warunek podmiany niespełniony.
Plan niezmieniony SHA256 cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec.
Historyczne szczegóły dev1–22 w STATUS.md i osobnych kontraktach/raportach.
Nadal otwarte: klient natywny, publiczny TLS/proxy, rzeczywisty import, długi soak,
kwalifikacja OS/dependencies i komplet metryk RSS/CPU/buforów.
Referencja gg-retro ed9640cc247764754b5e47891053eb07a53f76d5 w D:\Dokumenty\Documents\gg-retro.
Libgadu9eb2aa0eb6ad020fda7c7264538e48c7b78ba602 w workspace/rgg-reference-libgadu.
