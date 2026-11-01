# Punkt wznowienia — 2026-11-01, dev.24

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

Dev.23 zapisany w lokalnym commicie24e0343. Dev.24 aktualizuje SQLx0.9.0,
libsqlite3-sys0.37.0 i wbudowane SQLite3.51.3 (sprawdzony nagłówek oficjalnej paczki).
Przy otwarciu każdej puli SELECT sqlite_version() wymaga co najmniej3.51.3.
Statyczne migracje używają SqlSafeStr; bez zmiany12 skryptów SQL/sum ani schematu.
102 testy Rust, Clippy,23 regresje,9 grup awarii przeszły; normalne binarki
przywrócone. Nowy tests/sqlite_upgrade.py wymaga prawdziwych starych binarek:
kopię dev23 odtworzono starym adminem, potem nowy serwer zalogował stare konta,
odebrał offline i obrazek, odczytał kontakty/profil, przyjął nową wiadomość,
zrobił nowy backup/restore. Oryginały niezmienione. Baseline /tmp/rgg-baseline-dev23.
Inwentarz190, RustSec0/0 (1395 porad, commit2295466c4e9df26647fd67d8fa1c705ac2fc7f81).
SQLITE-UPGRADE.md, sqlite-upgrade/regression/faults-2026-11-01-dev24.json są dowodami.

Dev.24 zapisany w bf94fbd78d813f1a64e9192ba59dafebde4968b9.
Obraz dev24 przeszedł26 zestawów, restart/restore i R0:600s,100Ready,6000 wiadomości,
3000online/3000offline,1CPU/256MiB/swap0/PID128. ACKp95=4.535ms,onlinep95=9.180ms,
CPU0.115566,RSSpeak18522112B,cgrouppeak20459520B. Wszystkie bramki,6000TWU,
zero sesji/payloadów,integrity/FK oraz cleanup przeszły. Raport R0 nie jest soak.
Runtime sha256:054d8dd16f12e3a08cf11bc5aaf3a65064704f7c8c89cff8fea3b1a48eb7c016.
Verification sha256:6ee934bb355ecd7ff47909b4b12e2d1d806a5373dd5c6623412e2ed11d6dfcb0.
Server release68faf007173ab6667905e181d4617843714d85129a7a71309f0c887ac6f1609a.
Sourcearchive60ca04f9e4c39145115e1db4b7ed76a9ef1d64cf7f4332a4b9960bfb87cc280f.
Eksport workspace/rgg-container-2026-11-01-dev24/rgg-dev24-runtime.tar.gz,
61680178B,SHA95088150316b620c0812c19350981bcc05d1050bb6d925bb451f6df005432d37.
Manifest container-artifact-2026-11-01-dev24.json i6 surowych raportów skopiowano
do repo. Prywatny daemon zatrzymany po eksporcie (dawna sesja71330 wyszła0).

Kolejność dalszej pracy:
1. Zakończyć osobny lokalny commit dokumentacji kwalifikacji obrazu dev24.
2. Dokończyć rozpoczętą poprawkę klienta (planowane dev25). W worktree zmieniono
   tools/client_patch.py (3 dodatkowe zakresy,82B własnego kodu w128B paddingu),
   mapę przeglądarkową27 zakresów, test instalatora i liczby zakresów HTTP,
   tekst patchera. Dodano samodzielny MIT validation/gg6_upload_cpu/check.py,
   LICENSE,README,przypięty wheel Unicorna oraz tests/client_upload_wire.py.
   Nie usuwać tych niezacommitowanych zmian. Repo nadal ma wersjędev24.
3. Prototyp i pełny klient przeszły79 przypadków CPU (wszystkie granice2047,
   limit50000,NULL),8 rzeczywistych sekwencji TCP i restart. Pełny klient dla
   127.0.0.1 ma SHA b5e5c51f61467a07a0dce1079c45bfd4596cb3659aeb8a5b37843ad53faaaaeb.
   tests/client_package.py z oryginałem przeszedł5 testów+3 adresy+464 zasoby
   Windows LoadStringW bez startu GG. Node/JS vs Python:6 wektorów identycznych.
   Po ostatniej walidacji dodano tylko ograniczenie długości plików wejściowych
   do check.py; ponowić finalną walidację, aby report.driver_sha256 się zgadzał.
4. Zapisać dokumentację/proweniencję poprawki, bumpdev25, metadane Cargo,
   odpowiednie testy HTTP/Rust i lokalny commit. Następnie dalsze prace lokalne.
   Natywnego GG nadal nie uruchamiano. Żadnych podmian zainstalowanego klienta.

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
Daemon zatrzymany po dev24 (sesja71330 wyszła0). Przy przyszłych próbach zatrzymać tylko ten
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

SQLITE-UPGRADE.md oraz VERIFICATION-2026-11-01-dev24.md — najnowsze dowody.
PARITY.md rozróżnia implementację od kwalifikacji; warunek podmiany niespełniony.
Plan niezmieniony SHA256 cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec.
Historyczne szczegóły dev1–22 w STATUS.md i osobnych kontraktach/raportach.
Nadal otwarte: klient natywny, publiczny TLS/proxy, rzeczywisty import, długi soak,
kwalifikacja OS/dependencies i komplet metryk RSS/CPU/buforów.
Referencja gg-retro ed9640cc247764754b5e47891053eb07a53f76d5 w D:\Dokumenty\Documents\gg-retro.
Libgadu9eb2aa0eb6ad020fda7c7264538e48c7b78ba602 w workspace/rgg-reference-libgadu.
