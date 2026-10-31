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
