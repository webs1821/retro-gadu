# Punkt wznowienia — 2026-11-01, dev.18

Użytkownik wznowił prace i upoważnił do dalszej implementacji. Nadal wyłącznie
lokalnie: testy i commity jako webs1821, bez push i wdrożenia rGG. Branch master,
origin webs1821/retro-gadu nieużywany. Plan i recenzje pozostają niezmienione.
Nie uruchamiać nowych agentów bez zlecenia delegacji. Brak aktywnego celu/narzędzia goal.

Gotowe lokalnie: konta/zaproszenia (także stare kody), LOGIN60/GG32, sesje,
trwały tekst online/offline, obecność/prywatność, archiwa kontaktów, katalog,
discovery, WWW/rejestracja/liczniki, konferencje, klasyczne obrazki bez trwałego
cache oraz katalog pobierania kontrolowany manifestem. CLI ma pełne kopie,
verify/restore, rotację kluczy ze wznowieniem i importer gg-retro.

Aktualnie dev.18: schema 11, 10 modułów, 190 wersji zależności, 83 testy Rust.
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

Kolejne luki: upload kontaktów (odroczone ACK, pełny końcowy fragment 2047 B),
domena/profil wdrożeniowy, lokalny kontener i długi soak. Patcher jest przez CLI,
brak odpowiednika w przeglądarce. Limity HTTP są lokalnie sprawdzone, ale publiczny
TLS/reverse proxy i jego pochodzenie żądań wymagają osobnego profilu/kwalifikacji.
Docker.exe i /usr/bin/docker są dostępne. Konteksty default i desktop-linux
wskazują lokalne named pipes. Odczyt docker --context desktop-linux info odmówił
połączenia: named pipe nie istnieje, Docker Desktop nie ma uruchomionego silnika.
Nie uruchamiano Dockera ani żadnego kontenera. Nie używać domyślnego kontekstu bez
sprawdzenia endpointu; kolejny etap ma pozostać wyłącznie lokalny.
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
