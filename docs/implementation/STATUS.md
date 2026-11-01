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
