# Stan implementacji

## 2026-10-31 — E01: pierwszy działający fundament

Zakres: nowy lokalny workspace Rust (7 modułów), konfiguracja loopback,
wyłączna blokada katalogu, migracje SQLite i sprawdzane ustawienia WAL/FULL,
nowa epoka procesu, live/ready, zamknięcie po SIGTERM/Ctrl+C, ramki GG6,
parser LOGIN60, CP1250 i port GG32 z jawnym pochodzeniem.

Odniesienie do planu: E00 zakończony wcześniej; E01 zaimplementowany lokalnie.
Pełny zakres odnotowuje [PARITY.md](PARITY.md); postęp E02 opisano poniżej.
E08 nie jest upoważnieniem do wdrożenia: obowiązuje późniejsze ustalenie
użytkownika o rozwoju wyłącznie lokalnym do osiągnięcia zgodności.

Uruchomiono kompilację i testy na Linux/WSL z Rust 1.99.1. Test procesu używa
osobnej bazy w katalogu tymczasowym i sprawdza HTTP, drugi proces na tych samych
danych, SIGKILL/restart, nową epokę, migracje, integralność, SIGTERM i odrzucenie
publicznego nasłuchu. Test transportu przechodzi przez prawdziwe TCP oraz granice
protokół → aplikacja → protokół. Nie wykonywano testów klienta natywnego.

Nie wykonano operacji na Containers ani działającym gg-retro. Nie utworzono
zdalnego repozytorium. Autorem lokalnych commitów jest webs1821.

## 2026-10-31 — E02a: konta, sekrety i narzędzie lokalne

Wersja `0.1.0-dev.2`. Dodano moduły `rgg-credentials` i `rgg-admin`, port kont,
migracje 0002–0003 oraz testy. Koperty XChaCha20-Poly1305 wiążą sekret z UIN, wersją,
przeznaczeniem i key_id. Keyring ma losowy identyfikator klucza, kontrolę uprawnień,
tryb create-new, ograniczony rozmiar i test deszyfrowania próbki dla każdego klucza.
Nie ma regenerowania utraconego klucza istniejących kont ani trybu plaintext.
Historia par klucz–nonce pozostaje po usunięciu konta; kolizja przerywa całą
transakcję. Migrację istniejących kont do tej historii sprawdza osobny test.

Transakcja rejestracji obejmuje zużycie zaproszenia, wybór numeru i zapis konta.
Testy obejmują 12 równoczesnych zgłoszeń do kodu o trzech użyciach, konkurencję o
ręczny numer, pomijanie zajętych numerów, rollback po błędzie szyfrowania oraz
trwałą rezerwację UIN po usunięciu konta. CLI i serwer wzajemnie wykluczają dostęp
do danych. Testy CLI używają tylko tymczasowych baz i haseł syntetycznych.

Wynik: 28 testów Rust, dwa zestawy testów procesu/CLI, formatowanie, Clippy,
kontrola pochodzenia i audyt RustSec przeszły poprawnie. Audyt zgłasza 0 podatności
i 0 ostrzeżeń dla końcowego lockfile. Zapisano inwentaryzację 189 wersji bibliotek.
Szczegóły, wersje narzędzi i ograniczenia: [raport weryfikacji](VERIFICATION-2026-10-31.md).

E02 **nie jest jeszcze ukończony**: brak podłączonego logowania GG6, koordynatora
sesji, atomowego sprawdzania generacji przy skutkach komend oraz bramki gotowości
po synchronizacji prywatności. Biblioteka potrafi czytać stare klucze i pisać
aktywnym, ale operacyjna, wznawialna rotacja nie jest jeszcze zaimplementowana.

### Zadania zaplanowane po dev.2 (realizacja poniżej)

1. GG_LOGIN60 i niezależny klient testowy, bez przejmowania produkcyjnego klienta.
2. Koordynator aktorów kont, zatrzymanie starego writer-a przed nową sesją,
   warunkowe zatwierdzanie credential_version/session_generation.
3. Minimalny kontrakt prywatności przed `Ready` i testy wyścigów unieważnienia.
4. Trwała kolejka offline, ACK nadawcy po commit, FIFO, stan niepotwierdzonego
   odbioru GG6, recovery epoki i testy awarii/ponowień.
5. Kolejne funkcje z macierzy zgodności; klient natywny kwalifikowany osobno.

## 2026-10-31 — E02b / E03: logowanie, sesje i trwałe wiadomości GG6

Wersja `0.1.0-dev.3`, implementacja `93f8fa8`. LOGIN60/GG32 działa na osobnym
porcie loopback. Aktor konta zarządza sesją, zatrzymuje poprzedni transport
przed zmianą generacji i sprawdza credential_version. Skutki komend mają
atomowe sprawdzenie sesji/epoki w SQLite. Ready wymaga zakończenia listy
prywatności; blokady są zachowywane przy niepełnej synchronizacji i restarcie.

Migracja 0004 dodaje sesje, reguły kontaktów, wiadomości, dostarczenia i próby.
Tekst CP1250 oraz podstawowe style przechodzą przez trwałą kolejkę. ACK=3 wychodzi
po commit, FIFO nie zależy od seq ani zegara, a recovery nowej epoki odzyskuje
niezakończone lease. Zapis do TCP ma osobny stan bez obietnicy odbioru GG6.
Są limity, TTL, czyszczenie treści, ograniczona historia i izolacja wolnego klienta.

Wynik: 37 testów Rust, testy CLI i cyklu życia oraz pięć grup testów TCP.
Dodatkowy build testowy przeszedł cztery kontrolowane awarie na granicach commit
i wysyłki. Końcowy zwykły build również przeszedł wszystkie zestawy procesu.
Clippy, formatowanie, pochodzenie i RustSec: poprawnie; 0 znanych podatności
i 0 ostrzeżeń. [Pełny raport](VERIFICATION-2026-10-31-dev3.md).

Zakres tej prośby jest zaimplementowany i przetestowany lokalnie. E02 nadal
wymaga operacyjnej rotacji kluczy, a kwalifikacja klienta natywnego jest otwarta.
Kolejne funkcje to pełna obecność i listy kontaktów, następnie brakujące elementy
z macierzy. Warunek rozważenia podmiany gg-retro pozostaje niespełniony.

## 2026-11-01 — E04: obecność, prywatność i archiwum kontaktów

Wersja `0.1.0-dev.4`. Statusy, opisy CP1250, czas powrotu, niewidoczność,
tryb tylko znajomi oraz subskrypcje są podłączone do TCP. Wspólna bramka chroni
odczyt i publikację przed konkurencyjnym zatwierdzeniem zmiany prywatności.
Ready atomowo publikuje własny stan i reguły; niejawne rozłączenie nie utrwala
opisu online. Jawny offline przeżywa restart i respektuje politykę obserwatora.

Migracja 0005 dodaje obecność i wersjonowane archiwa kontaktów. Staging jest
odseparowany od poprzedniej opublikowanej listy i reguł prywatności. ACK wychodzi
po pełnym commit. Legacy upload z końcowym pełnym blokiem bez terminatora
pozostaje nierozstrzygalny: obowiązuje ograniczony profil i timeout bez utraty
starej listy. Nie oznaczamy rodzimego importu GG6 jako w pełni zakwalifikowanego.

44 testy Rust, cztery zestawy procesu/TCP, sześć kontrolowanych awarii i Clippy
przeszły poprawnie. [Raport dev.4](VERIFICATION-2026-11-01-dev4.md) oraz
[kontrakt](PRESENCE-AND-CONTACTS.md) określają dokładne granice potwierdzenia.
Kolejny przyrost: E05, katalog i WWW. Nadal wyłącznie lokalnie, bez wdrażania.

## 2026-11-01 — E05a: dobrowolny katalog publiczny

Wersja `0.1.0-dev.5`. READ/WRITE i SEARCH są podłączone do GG6. Profil jest
dobrowolny, pełny formularz zastępuje poprzedni, a pusty usuwa publikację. Regresje
zerowych sekwencji, odpowiedzi READ=3 oraz pustych wyników są objęte testami.
Filtry normalizują polskie znaki i uwzględniają pola rodzinne. Wyszukiwanie
korzysta ze wspólnej prywatności obecności oraz bariery publikacji.

Migracja 0006 dodaje profile i trwały limit 20 operacji / 10 sekund na konto.
Strony po 20 osób mają kursor po numerze, globalnie do 10000 profili. Odczyt
strumieniuje kandydatów. 50 testów Rust i pięć zestawów procesu/TCP przeszło;
Clippy i RustSec bez ostrzeżeń. [Raport](VERIFICATION-2026-11-01-dev5.md).

E05 nadal częściowy: WWW, discovery, samoobsługa i administrator online pozostają
kolejnymi przyrostami. Oryginalnego GG6 nie kwalifikowano. Rozwój nadal lokalny.

## 2026-11-01 — E05b: lokalne discovery HTTP

Wersja `0.1.0-dev.6`. Opcjonalne appmsg3/appmsg4 podają rzeczywisty lokalny port
GG6, obsługują HTTP/1.0 i nie ufają Host ani nagłówkom proxy. Brak Ready lub
wyłączona opcja zwracają notoperating. Stare operacje rejestracji, zmiany hasła
oraz przypominania odmawiają bez dotykania kont i zaproszeń.

52 testy Rust, HTTP discovery i odmowy, cykl życia oraz sesje TCP przeszły.
Clippy, kontrola pochodzenia i audyt bez uwag. [Raport](VERIFICATION-2026-11-01-dev6.md).
Kolejny przyrost E05: rejestracja WWW przez istniejącą transakcję zaproszenia,
z ochroną żądań i limitami. Nadal bez natywnej kwalifikacji i bez wdrożenia.

## 2026-11-01 — E05c: lokalna rejestracja WWW

Wersja `0.1.0-dev.7`. Opcjonalny formularz używa wspólnej, atomowej rejestracji
konta i zużycia zaproszenia. Wybór numeru, przydział od 112233, szyfrowanie hasła
i brak automatycznego wpisu publicznego zachowują kontrakt CLI. Origin/Host,
jednorazowy CSRF, monotoniczne limity oraz deadline chronią lokalny endpoint.

55 testów Rust, 4 grupy HTTP rejestracji i wszystkie regresje discovery, katalogu,
obecności/kontaktów, sesji, CLI i cyklu życia przeszły. W teście 12 równoczesnych
żądań powstały dokładnie 3 konta z jednego zaproszenia. Formularz obejrzano
w lokalnej przeglądarce desktop na tymczasowej bazie; podgląd zamknięto.
[Kontrakt](WEB-REGISTRATION.md), [wyniki](VERIFICATION-2026-11-01-dev7.md).

E05 nadal nie jest ukończony: samoobsługa, bezpieczne odzyskiwanie i publiczny
profil wdrożeniowy są otwarte. W macierzy brakuje też statystyk, konferencji,
obrazków i kwalifikacji. Rozwój pozostaje wyłącznie lokalny.

## 2026-11-01 — E05d: anonimowe statystyki i strona główna

Wersja `0.1.0-dev.8`. Opcjonalne `web_public_stats` udostępnia stronę główną
i JSON /api/stats. Liczniki pochodzą z jednej migawki bazy; obecność wymaga
aktualnej, gotowej sesji i przechodzi wspólną politykę anonimowego obserwatora.
Nie pobieramy identyfikatorów, opisów ani sekretów do wyliczenia online.

56 testów Rust i trzy grupy HTTP statystyk przeszły, wraz z regresjami
rejestracji, discovery i cyklu życia. Wygląd strony głównej sprawdzono w lokalnej
przeglądarce desktop; chwilowy proces i tab zamknięto. Reszta regresji ma pełny
wynik z dev.7. E05 nadal wymaga samoobsługi i publicznej kwalifikacji.
[Kontrakt](PUBLIC-STATISTICS.md), [wyniki](VERIFICATION-2026-11-01-dev8.md).

## 2026-11-01 — E06a: konferencje i trwały kontekst rozmowy

Wersja `0.1.0-dev.9`, schemat 7. Klient wykonuje fan-out; serwer zapisuje jedną
kopię na głównego adresata. Uczestnicy zachowują kolejność i formatowanie przez
kolejkę/restart, limit wynosi 32 osoby. Kontekst nie nadaje praw do innych kont.
Ochrona odbiorcy, kwoty i ACK pozostają takie jak w zwykłej wiadomości.

58 testów Rust, 3 grupy konferencji, rzeczywista migracja kolejki dev.8 oraz
6 kontrolowanych awarii przeszły. Pełna regresja wcześniejszych funkcji także
przeszła na końcowym zwykłym buildzie. Brak nowych zależności zewnętrznych.
[Kontrakt](CONFERENCES.md), [wyniki](VERIFICATION-2026-11-01-dev9.md).
E06 nadal wymaga obrazków i kwalifikacji klienta natywnego. Wszystko lokalnie.

## 2026-11-01 — obrazki GG6, dev.10

Schemat 8 i dziesiąty moduł `rgg-transfers`. Trwałe oferty w transakcji wiadomości,
klasyczne pobieranie od połączonego nadawcy, składanie/CRC w RAM, limity klienta
i serwera. Uprawnienia są zależne od odbiorcy, obu sesji i blokad; osobne bramki
publikacji chronią każdy zapis TCP. Historia wiadomości nie usuwa ofert.

68 testów Rust oraz nowe testy TCP/awarii/migracji; szczegóły w
[raporcie dev.10](VERIFICATION-2026-11-01-dev10.md) i [kontrakcie](IMAGES.md).
CRC korzysta z już obecnego crc 3.4.0, bez nowej zewnętrznej wersji biblioteki.
Plan i recenzje pozostają niezmienione. Brak push i wdrożenia rGG.

## 2026-11-01 — lokalny katalog pobierania, dev.11

Opcjonalny manifest z hashami, pochodzeniem i instrukcjami, strona /downloads,
JSON i pobrania. Weryfikacja przed startem, niezmienne kopie RAM, limity rozmiarów
i cztery strumienie. Bez dołączenia prawdziwych EXE, bez push/wdrożenia.

69 testów Rust, HTTP pobierania i regresje WWW/GG, desktop UI z dwoma sztucznymi
plikami. [Kontrakt i ograniczenia](DOWNLOADS.md), [raport](VERIFICATION-2026-11-01-dev11.md).

## 2026-11-01 — kompletne kopie i restore, dev.12

Offline backup-create/backup-verify/restore, spójny obraz SQLite z keyringiem,
manifest SHA-256 i pełna walidacja wszystkich sekretów. Brak nadpisywania celu,
znacznik niepełnego restore blokuje start. Dwie kontrolowane awarie przeszły,
podobnie jak odtworzenie pełnych danych i logowanie/odbiór obrazka po restore.
69 testów Rust, Clippy i audyt bez błędów. Brak nowych wersji zależności.
[Kontrakt](BACKUPS.md), [wyniki](VERIFICATION-2026-11-01-dev12.md). Nadal lokalnie.

## 2026-11-01 — rotacja kluczy, dev.13

keys-status/keys-rotate/keys-resume, obowiązkowa kompletna kopia przed rotacją,
publikacja pełnego keyringu przed zapisem rekordów, partie 64 i atomowa historia
nonce. Starsze klucze pozostają; hasła i credential_version nie zmieniają się.
71 testów Rust, w tym kolizja nonce i rollback całej partii, oraz trzy przerwania
procesu i logowanie przed/po wznowieniu. Starszy backup odtwarza się niezależnie.
Regresje backup/CLI/kolejki, Clippy i RustSec poprawne; normalny build bez hooków.
[Kontrakt](KEY-ROTATION.md), [wyniki](VERIFICATION-2026-11-01-dev13.md).

## 2026-11-01 — inspekcja źródła przed importerem

Odczyt wyłącznie kodu przypiętego forka 0.8.3: 9 tabel, 11 migracji, brak historii
usuniętych UIN i inne kodowanie zaproszeń. Nowe inspect_gg_retro.py generuje
raport liczników i luk, bez zmiany źródła, celu lub ujawniania sekretów.
Testy Windows i Linux przeszły, łącznie z odmową zmienianego źródła.
To jeszcze nie importer/dry-run konwersji; migration_ready zawsze false.
[Zakres, mapowanie i wyniki](IMPORT-GG-RETRO.md). Binarki pozostają dev.13.

## 2026-11-01 — stare zaproszenia, dev.14

Schemat 9: osobne legacy_invitations, SHA-256 dokładnego starego kodu, zachowanie
used/active i atomowej rejestracji. WWW/CLI, odmowa przejścia wyczerpanego kodu
rGG do starej tabeli. 72 testy Rust, trzy scenariusze adaptera, rzeczywista
migracja z dev.13 oraz odtworzenie starej kopii przez dev.13 → migracja do dev.14.
Pełny importer nadal nie powstał. [Kontrakt](LEGACY-INVITATIONS.md),
[weryfikacja](VERIFICATION-2026-11-01-dev14.md). Lokalnie, bez push i wdrożenia.


## 2026-11-01 — pełny lokalny importer gg-retro, dev.15

Schemat 10: receipt i prywatne metadane źródła. Read-only eksport z przypiętej
kopii 0.8.3, pełna walidacja AEAD/relacji/limitów przed celem, świeże szyfrowanie
i jedna transakcja. Zachowane numery, zaproszenia, kolejka i oferty, profile,
kontakty i zapisane opisy/prywatność; jawne decyzje o brakach i retencji.
Znacznik niepełnego importu powstaje przed bazą i blokuje start do końca kontroli.

74 testy Rust, integracja CLI/SQLite/TCP, trzy awarie procesu i powtórzenie do
nowego katalogu. Migracja rzeczywistych binarek dev.14 oraz restore schematu 9
przeszły, podobnie jak pełna regresja funkcji. Brak nowych zewnętrznych wersji
bibliotek, 0 podatności/ostrzeżeń RustSec. Wszystkie dane syntetyczne.
[Kontrakt](IMPORT-GG-RETRO.md), [wyniki](VERIFICATION-2026-11-01-dev15.md).


## 2026-11-01 — uwierzytelniona zmiana hasła offline, dev.16

Schemat 11 z ograniczonym audytem. CLI i interaktywny pomocnik przyjmują pełne
obecne hasło; świeży szyfrogram, nonce, credential_version i audyt mają jeden
commit. Konta, dane rozmów, profile, kontakty i zaproszenia pozostają zachowane.
77 testów Rust, rzeczywiste logowania GG6 i odtworzenie kopii sprzed/po zmianie,
dwa przerwania procesu oraz migracja i restore dev.15 przeszły. Importer wraz
z trzema awariami nadal działa. [Kontrakt](PASSWORD-CHANGE.md),
[raport](VERIFICATION-2026-11-01-dev16.md). Nadal lokalnie, bez push/wdrożenia.

Przegląd referencji list kontaktów nie rozstrzygnął końca pełnego bloku dla
zamkniętego GG6; libgadu wysyła fragmenty bez czekania na ACK. Nie zmieniano
ograniczonego profilu uploadu na podstawie heurystyki.


## 2026-11-01 — lokalny generator klienta i instalatora

Wersja `0.1.0-dev.17`, schema 11 bez zmian. Niezależny patcher PE/NSIS dla
przypiętego oryginalnego GG6.1: adres z konfiguracji, wyłączony aktualizator,
poprawne nazwy dźwięków oraz przebudowa tablic tekstów bez ucinania kolejnych
zasobów. Zachowany stub, skrypt/licencja i pozostały ładunek instalatora.
Nowy katalog publikowany manifestem dopiero po sprawdzeniu zapisanych plików.

Przeszło pięć testów syntetycznych, trzy hosty na rzeczywistym instalatorze,
464 odczyty zasobów Windows dla każdego wariantu, test pobierania obu artefaktów
przez lokalny HTTP oraz build/lifecycle dev.17. Kod Rust poza wersją nie zmieniony;
77 testów Rust pozostaje wynikiem dev.16. Audyt 0 podatności/ostrzeżeń.
Klienta i instalatora nie uruchamiano, niczego nie wdrożono ani nie pushowano.
[Kontrakt](CLIENT-PACKAGES.md), [raport](VERIFICATION-2026-11-01-dev17.md).

## 2026-11-01 — ograniczony transport HTTP i lokalny pomiar

Wersja `0.1.0-dev.18`, schema 11 bez zmian. Dodano globalne/per-IP limity
przyjmowania gniazd, bezczynność, ograniczenie zablokowanego zapisu i maksymalny
czas połączenia. Terminy obejmują body pobierania, a zwolnienie socketu oddaje
slot pliku. Dane od klienta i pusty flush nie utrzymują zablokowanego zapisu.

83 testy Rust, Clippy i trzy testy realnych gniazd przeszły; także regresja
wszystkich funkcji HTTP i sesji GG. Audyt: 0 podatności i ostrzeżeń. Własna
trzyminutowa próba zweryfikowała 4175 wiadomości w 167 cyklach online/offline,
180 pobrań pliku i 540 oczekiwanych ograniczeń 429. RSS maks. 24,1 MiB; baza
poprawna i proces zamknięty czysto. Wynik nie zastępuje długiego soak ani pomiarów
profilu produkcyjnego. Nadal wyłącznie lokalnie, bez push i podmiany usług.
[Kontrakt](HTTP-LIMITS.md), [raport i surowe pomiary](VERIFICATION-2026-11-01-dev18.md).

## 2026-11-01 — obraz lokalny i kwalifikacja kontenera

Runtime nadal dev.18. Z przypiętych baz i commitu `7bc2d66` powstał obraz zawierający
release binarki oraz archiwum kompletnych źródeł z vendorami. Narzędzie buduje
kontekst wyłącznie z Git, pomijając nieśledzone dane. Sprawdzono to na Windows
oraz WSL, także po poprawce dla starego Git i różnicy właściciela katalogu.

20 zestawów regresji przeszło na binarkach obrazu. Osobna próba potwierdziła
nieuprzywilejowany PID1, rzeczywisty limit RAM/CPU, trwały wolumen, wiadomość
po restarcie kontenera oraz backup/verify/restore do nowego wolumenu. Wynik
zawiera ważne ograniczenie: WSL nie zastosował żądanego limitu swapu. R0,
klient natywny, publiczny profil, długi soak i skan OS nadal są otwarte.

Archiwum obrazu zapisano lokalnie (61,1 MB), a testowe dane/usługi zakończono.
Tymczasowy Docker 29.9.2 nie zastąpił systemowych narzędzi ani Docker Desktop.
Nie było push, publikacji ani połączenia z produkcyjnym Containers.
[Kontrakt](CONTAINERS.md), [raport](VERIFICATION-2026-11-01-container.md),
[manifest artefaktu](container-artifact-2026-11-01-dev18.json).

## 2026-11-01 — prywatne metryki operacyjne (dev.19)

Osobny opcjonalny listener loopback eksportuje 12 rodzin/19 szeregów gauge:
stan kont i bieżących sesji, kolejkę i terminalne rekordy dostarczeń, logiczne
bajty/wiek kolejki, archiwa oraz niewygasłe granty. Agregaty powstają w jednym
odczycie SQLite, z krótkim cache i deadline; brak etykiet/danych użytkowników.
Publiczne liczniki zachowują dotychczasową politykę niewidoczności. Listener
ma osobne limity i uczestniczy we wspólnym uruchamianiu/zamykaniu usług.

88 testów Rust, Clippy, realne HTTP/GG metrics i regresja sesji/lifecycle/stats/
limitów przeszły. Schema nadal 11, 190 zależności, 8 wpisów pochodzenia. Nie
rozszerzano kwalifikacji wcześniejszego obrazu dev.18 na nową binarkę.
[Kontrakt](METRICS.md), [raport](VERIFICATION-2026-11-01-dev19.md).


## 2026-11-01 — obraz dev.19 i zakończona krótka próbka R0

22 zestawy regresji release przeszły. Pełna próba 100 sesji/10 wiadomości na
sekundę przez 180 s, z rzeczywistym CPU1/RAM256MiB/swap0, spełniła wszystkie
wcześniej ustalone progi. Sprawdzono 1800 payloadów, w tym 900 offline/FIFO,
końcowy stan i integralność DB, shutdown i cleanup. ACK p95 3,992 ms, online
p95 7,621 ms, RSS peak 15,91 MiB, CPU średnio 9,71% jednej vCPU.

Zachowano także nieukończone przygotowania i ich wyjaśnienia. Poprawki były
w sterowniku: właściciel pustego wolumenu, tempo logowania, odczyt kopii WAL.
Kod serwera i progi nie wymagały zmian. Archiwum obrazu, manifest i raporty
zapisano na komputerze; prywatny daemon wyłączono po sprawdzeniu cleanup.
Długi soak, klient natywny i pełna kwalifikacja produkcyjna pozostają otwarte.
[Raport](VERIFICATION-2026-11-01-r0.md), [scenariusz](R0-LOAD.md).


## 2026-11-01 — dev.20: przygotowanie GG6 w przeglądarce

Dodano opcjonalny lokalny formularz, niezależny moduł ES i mapę 24 zakresów
z przypiętego oryginału. Zmiany adresu/updatera/dźwięków wykonywane wyłącznie
w przeglądarce, z pełnym SHA-256, zachowaniem RT_STRING i snapshotem wejścia.
Bez uploadu, podmiany zainstalowanego GG, push i wdrożenia. Włączenie wymaga
[client_patcher]; ustawienia sprawdzane przed otwarciem danych. Pozostałe
strony zachowują CSP bez JS. Nie zmieniono schematu 11 ani bibliotek.

89 testów Rust, Clippy, 6 porównań JS/Python całych plików oraz HTTP patchera
i regresje współdzielonych stron przeszły. W rzeczywistej przeglądarce odrzucono
zły plik, przygotowano i pobrano poprawny, porównano hash z dysku i sprawdzono
usunięcie starego linku po zmianie adresu. Natywnego klienta nie uruchamiano.
Podgląd wyłączony. Obraz/R0 nadal odnosi się do dev.19; nie powtarzano go.
[Kontrakt](BROWSER-PATCHER.md), [weryfikacja](VERIFICATION-2026-11-01-dev20.md).

## 2026-11-01 — dev.21: zmiana hasła przez WWW

Dodano samodzielny opcjonalny formularz z pełnym obecnym hasłem, trwałym limitem
5 prób/10 minut i oddzielnym celem CSRF. Komenda aktora zatrzymuje stary
transport przed zmianą sekretu; transakcja obejmuje nonce, wersję, sesję,
zwolnienie lease i audyt. Stary dowód logowania nie wyrzuca nowszej sesji.
Migracja 11→12 zachowuje historię i najwyższe ID nawet przy pustym audycie.

95 testów Rust, Clippy i 21 grup regresji rzeczywistych HTTP/TCP/CLI przeszły.
Sprawdzono dwie awarie procesu, kopie przed/po zmianie oraz prawdziwą migrację
ze starszej binarki dev.20, również po restore starego archiwum. Formularz
obejrzano w przeglądarce; zapis testowano klientem HTTP na danych syntetycznych.
Podgląd zatrzymany. Brak push, zmian produkcji i nowej kwalifikacji obrazu.
[Kontrakt](PASSWORD-WEB.md), [raport](VERIFICATION-2026-11-01-dev21.md).

## 2026-11-01 — dev.22: liczniki i histogramy runtime

Dodano stałe liczniki przyjęcia/kończenia połączeń i wyników login/accept/delivery
oraz histogramy ich czasu monotonicznego. Prywatny eksport ma teraz 17 rodzin
i 112 szeregów, poniżej 16 KiB. Snapshot jest ograniczony i spójny, bez I/O
w mutexie i bez danych użytkowników. Anulowanie zadania jest rejestrowane,
a socket, fence i gauge zwalniane także przy abort. Stan resetuje się z procesem.

100 testów Rust, Clippy, 16 grup regresji i 5 zestawów awaryjnych przeszły.
Slow reader wskazał, że trzeba objąć pomiarem także claim; poprawka zweryfikowana
na rzeczywistym TCP, z Interrupted i brakiem pozostawionego Leased. Stary limit
czytnika R0 zwiększono z 8 do 16 KiB, zachowując progi wydajności i poprzedni
wynik w raporcie. Końcowe testy runtime/HTTP/smoke powtórzone na zwykłym buildzie.
Schema12 i 190 bibliotek bez zmian. Brak push, wdrożenia oraz nowego obrazu/R0.
[Kontrakt](RUNTIME-METRICS.md), [raport](VERIFICATION-2026-11-01-dev22.md).


## 2026-11-01 — dev.23: kolejność zapisów SQLite

Dłuższy test obrazu dev22 (600s/6000 wiadomości) ujawnił rozłączenie odbiorcy
przy równoczesnym opróżnianiu3000 offline. Sama kolejka bez wcześniejszej historii
przeszła; po dodaniu3000 zakończonych dostarczeń odtworzono delivery timeout.
Wprowadzono1 zapisujący i3 połączenia tylko do odczytu, bez zwiększania limitów.
Kandydat release odebrał całą kolejkę w22.516s pod CPU1/RAM256MiB/swap0;
po SIGTERM wszystkie6000 były zapisane, bez sesji/payloadów i błędów integralności.
101 testów Rust,23 grupy TCP/HTTP/CLI i9 grup kontrolowanych awarii przeszły.
Nowy test regresji w CI i suite kontenera. Schema12,190 zależności i plan bez zmian.
Binarki przywrócono bez hooków. [Kontrakt](SQLITE-WRITER.md),
[raport](VERIFICATION-2026-11-01-dev23.md). Brak push i wdrożenia.

Podczas sprawdzania dokumentacji WAL potwierdzono potrzebę aktualizacji
wbudowanego SQLite3.46.0 z poprawką WAL-reset. To osobne zadanie przed końcową
kwalifikacją; nie jest dowodem uszkodzenia baz w opisanym teście.
Analiza funkcji uploadu GG6.1 i13 przypadków emulatora CPU potwierdziły brak
pustego terminatora dla wielokrotności2047 i limit klienta50000B. Poprawka pakietu
klienta jest kolejną pracą; aplikacji Windows nie uruchomiono.


## 2026-11-01 — dev.24: aktualizacja SQLite i kontrola zgodności danych

SQLx0.9.0 i libsqlite3-sys0.37.0 (wbudowane SQLite3.51.3, poprawka WAL-reset).
Magazyn sprawdza wersję rzeczywistego silnika, minimum3.51.3. Osadzone statyczne
migracje dostosowano do SqlStr; treść SQL,12 sum migracji i schema12 bez zmian.
102 testy Rust, fmt, Clippy,23 grupy regresji i9 grup --faults przeszły.
Przywrócono zwykłe binarki. Inwentarz190 zależności zgodny; RustSec0/0.
Osobny test przyjął prawdziwy backup poprzednich binarek dev23: stare hasła,
wiadomości offline, obrazek, kontakty i profil działały; nowy zapis i backup/restore
również. Oryginał i stare archiwum niezmienione. [Kontrakt](SQLITE-UPGRADE.md)
i surowe raporty sqlite-*-2026-11-01-dev24.json. Obraz/R0 nadal do wykonania.
Brak push, wdrożenia i kwalifikacji natywnego klienta.


## 2026-11-01 — kwalifikacja obrazu dev.24

26 grup release, restart/wolumen/backup-restore oraz pełny dłuższy R0 przeszły.
100 Ready,600s,6000 wiadomości,3000 online i3000 offline z dokładnymi payloadami
oraz FIFO. ACK p95 4.535ms, online p95 9.180ms, CPU11.5566%, RSS peak17.66MiB;
1CPU/256MiB/swap0/PID128. Po SIGTERM integralność/FK ok,6000TWU,zero sesji/payloadów.
Archiwum61680178B:95088150316b620c0812c19350981bcc05d1050bb6d925bb451f6df005432d37.
Źródło bf94fbd78d813f1a64e9192ba59dafebde4968b9, manifest i surowe raporty zapisane
w repo i workspace/rgg-container-2026-11-01-dev24. Prywatny daemon zatrzymany.
Długi soak/natywny GG/publiczny profil nadal otwarte. Brak push/deploy.


## 2026-11-01 — dev.25: zakończenie uploadu kontaktów GG6.1

Generator klienta i instalatora oraz patcher przeglądarkowy dodają pusty MORE
po dodatniej wielokrotności 2047 bajtów i zwiększają licznik oczekiwanych ACK.
Własny kod x86 zachowuje limit 50000 B, pusty PUT i ABI. Oryginalny klient bez
poprawki nadal nie ma jednoznacznego końca; serwer nie stosuje heurystyki czasu.
Trzy nowe zakresy dają łącznie 27. Schema 12 i zależności Rust bez zmian.

Samodzielny walidator MIT z zewnętrznym Unicornem: 79 przypadków CPU, a następnie
8 przechwyconych sekwencji przez TCP, atomowość archiwum i restart. 102 testy Rust,
fmt, Clippy, 3 regresje TCP/HTTP, parser pakietu, 3 adresy, 464 zasoby Windows,
6 porównań JS/Python oraz pobieranie gotowych plików przeszły. Zapisano pochodzenie
rozwiązania i oddzielnego narzędzia w THIRD_PARTY.toml (10 wpisów).
Pakiet dla 127.0.0.1 w workspace/rgg-client-local-2026-11-01-dev25.
Całej aplikacji/instalatora nie uruchomiono, istniejące GG i serwery bez zmian.
Obraz/R0 pozostaje dev.24; brak push i wdrożenia.
[Kontrakt](CLIENT-UPLOAD.md), [weryfikacja](VERIFICATION-2026-11-01-dev25.md).


## 2026-11-01 — dev.26: sprzątanie przy pełnej historii

Próba 100000 zakończonych dostarczeń + 200 offline ujawniła EOF w dev25:
109 odebranych, 91 nadal Pending, bez naruszenia integralności bazy. Po zmianie
indeksów/zapytań ten sam scenariusz odebrał 200/200 w 2.937 s. Zachowano oba
raporty. Migracja 13 dodaje messages_with_payload; wygasanie i prywatność
przechodzą po aktywnej kolejce, a purge po nadal przechowywanych payloadach.
Usunięcie konta czyści również conference w tej samej transakcji.

Nowy test pełnej historii obejmuje nowe przyjęcie, restart i retencję; dodany
w CI oraz suite kontenera. 103 testy Rust, fmt/Clippy, 25 grup regresji i 9 grup
awarii przeszły. Przywrócono normalne binarki. Prawdziwa kopia dev25 została
odtworzona starym adminem i zmigrowana do13, z zachowaniem danych i12 sum.
Nie zmieniono zależności, planu ani limitów czasu/retencji. Ostatni obraz/R0 to
nadal dev24. Brak push/deploy. [Kontrakt](HISTORY-RETENTION.md),
[raport](VERIFICATION-2026-11-01-dev26.md).
