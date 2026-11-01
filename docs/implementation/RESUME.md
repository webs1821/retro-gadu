# Punkt wznowienia — 2026-11-01, dev.20

## Najnowsza implementacja: patcher w przeglądarce

Dev.20, schema11, 10 modułów, 190 wersji zależności, 89 testów Rust.
Opt-in [client_patcher] default_host: 1..16 ASCII alnum z pojedynczymi . / -.
Strona /patcher i 3 osadzone assets. Własny ES module: pełny hash oryginału,
24 zakresy (3 RT_STRING, 17 Cstrings, 4 updater/sounds), snapshot wejścia/mapy,
Blob URL download, bez uploadu i zmian zainstalowanego klienta. Generator mapy
tools/client_plan.py korzysta z przypiętego instalatora jako danych (limit2MiB).
Node24.11.1/Python3.13: 6 porównań całego pliku, błędne dane i async mutation.
Rust89, Clippy, HTTP patcher/downloads/stats/register/metrics/lifecycle przeszły.
IAB: błędny plik odrzucony, właściwy przygotowany/pobrany, zmiana hosta usuwa
stary link, brak błędów JS. Pobrano testowy E:\Downloads\gg.exe (127.0.0.1),
hash ddbc24fc91ffbb5e4bd29ff628436e60a8eb1ca4a7cd617bad3e32910bdf541f;
nie uruchomiono. Podgląd localhost zatrzymany, karta zamknięta.
Debug server SHA1eccd3201968cc4244f8e72c104a7b3322f17d632a80aa3fca6778551e6704d1.
Kontrakt BROWSER-PATCHER.md, raport VERIFICATION-2026-11-01-dev20.md.
Nie budowano obrazu dev20: ostatni sprawdzony obraz/R0 pozostaje dev19 poniżej.

## Zakończony obraz dev.19 i krótka próba R0

Źródło obrazu 0b251ba42df7197b8fdc07a5855744a70c5c570f, runtime
sha256:f593f3c113b1dece0fd60c05f3df47b621c2733cb29ab5624bd2e149d745e105,
verification sha256:2c40b48b6fc2b8ca6840d0587af4e9d5762a6ad56bfaa4cd6dbaafdf63096618.
22 zestawy regresji release przeszły (63.316 s). Pełny R0: 100 sesji,
1800 wiadomości/180.001s (900 online + 900 offline z FIFO), ACK p95 3.992ms,
online p95 7.621ms, RSS peak 16678912 B, CPU 9.7075% jednej vCPU. Wszystkie
progi z R0-LOAD.md spełnione. Limity rzeczywiste: CPU1/RAM256MiB/swap0/PID128;
swap ustawiony bezpośrednio w cgroup własnego zweryfikowanego kontenera.
Po exit0 sprawdzono kopię DB+WAL: integrity/FK, zero sesji/payloadów, 1800
TransportWrittenUnconfirmed. Cleanup potwierdzony, zero kontenerów/wolumenów.
Prywatny daemon Docker29 ZATRZYMANY (session23702 zakończona exit0), brak zadań.
Raporty wcześniejszych nieukończonych przygotowań i krótkiej kontroli zachowano.
Sterowniki poprawiono bez zmiany runtime/progów: właściciel wolumenu z obrazu
runtime; logowanie co100ms z limitem64 w serii i16 nowych połączeń/s/IP; read-only DB/WAL
kopiowane po shutdown do tmpfs przed integrity_check (nie używać immutable=1).
Pełny raport r0-2026-11-01-dev19.json, opis VERIFICATION-2026-11-01-r0.md.
Runtime release SHA74f509e569400dade1da1fb365520cad115aea989b83fb55516c32c5523e750b.
Źródła/vendor archive SHA8a91a28af7aa84ad2ce94cb612e7d03e493f0b3a83330c54a747bc236fc4b85b.
Eksport rgg-dev19-runtime.tar.gz (61229424 B), SHA
ed9c16e3a757d7597c6194e75bfc89215c01733ee555451c642953f775a5906b,
w D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-container-2026-11-01-dev19.
Są tam manifest i wszystkie raporty; starsze artefakty dev18 pozostają.
Nadal brak długiego soak, natywnego GG6, publicznego profilu i skanu OS.
Nie uruchamiano Windows Sandbox (WindowsSandbox.exe nie ma na tym komputerze).

## Stan implementacji

Użytkownik wznowił prace i upoważnił do dalszej implementacji. Nadal wyłącznie
lokalnie: testy i commity jako webs1821, bez push i wdrożenia rGG. Branch master,
origin webs1821/retro-gadu nieużywany. Plan i recenzje pozostają niezmienione.
Nie uruchamiać nowych agentów bez zlecenia delegacji. Brak aktywnego celu/narzędzia goal.
Zlecony odczyt hasła konta 742858 wykonano na początku tury i wynik przekazano
użytkownikowi w czacie. Nie ponawiać odczytu; hasła nie zapisano w repozytorium.

Gotowe lokalnie: konta/zaproszenia (także stare kody), LOGIN60/GG32, sesje,
trwały tekst online/offline, obecność/prywatność, archiwa kontaktów, katalog,
discovery, WWW/rejestracja/liczniki, konferencje, klasyczne obrazki bez trwałego
cache oraz katalog pobierania kontrolowany manifestem. CLI ma pełne kopie,
verify/restore, rotację kluczy ze wznowieniem i importer gg-retro.

W dev.19: schema 11, 10 modułów, 190 wersji zależności, 88 testów Rust.
Dodano opt-in metrics_bind na osobnym loopback (niedozwolony ten sam endpoint
co HTTP/GG). Private /metrics: 12 rodzin/19 gauge, stałe etykiety, aktywne/
wyłączone konta, retired, valid ready/synchronizing z niewidocznymi, stany
retencji dostarczeń, logiczne bajty i wiek kolejki, bajty archiwów, niewygasłe
grants. Jeden read snapshot, cache <=500ms, max 2 żądania i 1 odczyt, deadline1s;
503 przy braku Ready/błędzie/timeout, 429 przy zajętości. Host/Origin/fetch-site
sprawdzane, query/body/TE odrzucane, proxy headers ignorowane. Osobny listener
8 gniazd/4 na IP, idle/write5s, lifetime60s; uczestniczy w shutdown/select.
Testy Rust, Clippy, real HTTP/GG metrics oraz regresja lifecycle/sessions/stats/
HTTP limits przeszły. Binarka WSL debug (bez test-hooks) SHA256
93ce00f9303922c0030b3fb00d970cd78e7992f7148e811f3243d10e1a1368c5.
Osiem wpisów pochodzenia (dodano Prometheus format jako reference-only).
Brak pełnych histogramów/counterów protokołu i buforów; nie uruchamiano
Prometheusa. Obraz dev.19 przeszedł 22 zestawy i krótki R0, opisane powyżej.
Obraz dev.18 sprawdzano sterownikiem z commitu 7033971 (20 pozycji).
Kontrakt METRICS.md i raport VERIFICATION-2026-11-01-dev19.md.
Dev.15 dodał pełny importer (commit 6b5b219):
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

Dev.16: password-change wymaga pełnego obecnego hasła, zapisuje świeży nonce,
wersję uprawnień i audyt w jednej transakcji. Password-audit pokazuje do 100 wpisów;
retencja widoku 30 dni, limit DB 10000. Brak resetu i endpointu online.
Nowe/stare logowanie, kolejka/obrazek, stare/nowe kopie, dwa przerwania oraz
migracja/restore dev.15 → dev.16 przeszły. [Kontrakt](PASSWORD-CHANGE.md),
[raport](VERIFICATION-2026-11-01-dev16.md). Normalne binarki bez hooków.

Przejrzano ponownie upload kontaktów: przypięte libgadu wysyła wszystkie fragmenty
przed ACK; nie zmieniono protokołu bez dowodu oryginalnego GG6. Pełny końcowy blok
pozostaje ograniczeniem.

Dev.17: własne tools/client_format.py, client_patch.py i build_client.py tworzą
osobny pakiet z przypiętego gg-retro/server/static/gg61.exe (SHA256
bcc8157aa6bface009d8018c308bf3cef8725546b4f826bdbaf6bbeaa953b06f).
Adres konfigurowalny do 16 znaków, poprawne tablice RT_STRING, wyłączony updater,
dźwięki wiadomosc/dostepny, zachowany cały pozostały rozpakowany instalator.
Nie skopiowano implementacji gg-retro. Pięć testów syntetycznych, trzy hosty,
464 zasoby LoadStringW dla każdego oraz prawdziwy katalog HTTP przeszły.
Windows mapuje EXE wyłącznie jako dane; klient i instalator nie były uruchamiane.
Katalog wynikowy: D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-client-local-2026-11-01,
adres 127.0.0.1; nic nie podmieniono. [Kontrakt](CLIENT-PACKAGES.md),
[raport](VERIFICATION-2026-11-01-dev17.md). Python pakowania wymaga >=3.11.

Dev.18: BoundedListener/BoundedIo w rgg-server ogranicza HTTP przed parserem:
64 gniazda/16 na faktyczne IP, idle 15s, zablokowany zapis 10s, lifetime 600s.
Osobne rejestracje timerów odczytu/zapisu; incoming/flush nie przedłuża write.
Sześć nowych testów Rust, trzy realne testy gniazd i regresja HTTP/GG przeszły.
Clippy all-features i RustSec poprawnie. Zwykłe binarki bez test-hooks.
Próba 180s: 4175 wiadomości online/offline, 167 cykli, szczyt RSS 24,1 MiB,
180 pobrań/540 oczekiwanych 429, integralność DB i czysty SIGTERM.
To nie długi soak ani dowód plateau RAM. [Raport](VERIFICATION-2026-11-01-dev18.md).

Kontener runtime dev.18 z SHA 7bc2d669a6039b0c770186d781edbc537dce2f9e jest
zbudowany i sprawdzony lokalnie. 20 zestawów kontroli na release binarkach oraz
osobny PID1/UID10001/cgroup/volume/restart/backup-restore przeszły. Tymczasowe
kontenery/wolumeny usunięto, a prywatny daemon zakończył się kodem 0.
Runtime image sha256:b0f5d9eae97ba7d4c12e2e068730e48ebf3e7a75ed7a0ad3fa84fca0102673ab;
test image sha256:cf3a7b38b9bb927ad7a83dfef102f8c4cb498e3bdbf7cb54ad1bc54a7d995ffe.
Binarka release ecc6b15263b9363014bd69fa46cae839907cf30c9d97ee031bcb1c4ed5b529c0.
Raport VERIFICATION-2026-11-01-container.md i container-artifact-2026-11-01-dev18.json
wiążą źródła, fixture, lockfile, image ID, binarkę i hashe sterowników.
Archiwum 61115802 B: D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-container-2026-11-01\rgg-dev18-runtime.tar.gz,
SHA256 d4cc60eba8bff1445c4ade5ed7bae43810daf5a0897194e5a04eff3c79b770fa.
Źródła wraz z vendorami/licencjami są także wewnątrz obrazu. Compose sprawdzony
przez config, bez uruchamiania nim instancji. R0 100 sesji, natywny GG6, skan OS
i długi soak NIE są zakwalifikowane. Memory.max 256 MiB i 1 CPU były skuteczne,
ale memory.swap.max = max pomimo żądanego --memory-swap; swap WSL aktywny.

Kolejne luki: upload kontaktów (odroczone ACK, pełny końcowy fragment 2047 B),
domena/profil wdrożeniowy, metryki/R0 i długi soak. Patcher jest przez CLI,
brak odpowiednika w przeglądarce. Limity HTTP są lokalnie sprawdzone, ale publiczny
TLS/reverse proxy i jego pochodzenie żądań wymagają osobnego profilu/kwalifikacji.
Docker Desktop pozostawiono nieuruchomiony. Konteksty default i desktop-linux
wskazują lokalne named pipes, ale silnik Desktop nie działa. Do prób użyto
odizolowanego WSL /tmp/rgg-docker-local-20261101 (root, 0700): narzędzia Docker
29.9.2/runc 1.5.2 w bin/, dane data29/, config daemon29.json, prywatny docker29.sock.
Daemon jest teraz ZATRZYMANY; pliki/cache zostały do późniejszego użycia. Pierwotny
tymczasowy daemon systemowego Docker26 również zatrzymany; nie uruchomił kontenerów.
Nie zmieniano programów systemowych ani firewalla/forwardingu/mostków.
Docker CLI zawsze z jawnym --host unix://... i pustą własną konfiguracją; nie
używać domyślnego kontekstu. Statyczne narzędzia są tylko do lokalnych prób.
W WSL Git2.25.1 nie honoruje nowszego GIT_CONFIG_GLOBAL tak jak bieżący Git:
build_container.py używa jawnego git-dir/work-tree, bez zmian globalnej konfiguracji.
Obraz buduje wyłącznie z commitów, nie ze zmienionego worktree. Raporty są
kwalifikacją artefaktu z 7bc2d66, nie każdej późniejszej rewizji narzędzi/dokumentacji.
Natywny GG6/Windows nadal niekwalifikowane. Do czasu parytetu nie rozważać podmiany.
[PARITY](PARITY.md) zawiera aktualne rozróżnienie kodu i kwalifikacji.

Repo: D:\Dokumenty\Documents\RetroGadu, poza writable roots — polecenia mutujące
wymagają require_escalated z konkretnym uzasadnieniem. Git author webs1821.
WSL: CARGO_HOME=/tmp/gg-retro-toolchain/cargo,
RUSTUP_HOME=/tmp/gg-retro-toolchain/rustup, CARGO_TARGET_DIR=/tmp/rgg-target,
CC=/tmp/gg-retro-toolchain/gcc/cc; dodać CARGO_HOME/bin do PATH.
Binarki: /tmp/rgg-target/debug/rgg-server i rgg-admin, zwykłe bez test-hooks.
Baselines: /tmp/rgg-baseline-dev8, dev9, dev13, dev14 i dev15 (osobne katalogi).
Python Windows 3.13 do tomllib/kontroli repo, WSL 3.8 do TCP/process. W WSL brak rg.
Referencja libgadu: 9eb2aa0eb6ad020fda7c7264538e48c7b78ba602,
D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-reference-libgadu.
gg-retro reference: D:\Dokumenty\Documents\gg-retro, ed9640cc247764754b5e47891053eb07a53f76d5.
