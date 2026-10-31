# Stan implementacji

## 2026-10-31 — E01: pierwszy działający fundament

Zakres: nowy lokalny workspace Rust (7 modułów), konfiguracja loopback,
wyłączna blokada katalogu, migracje SQLite i sprawdzane ustawienia WAL/FULL,
nowa epoka procesu, live/ready, zamknięcie po SIGTERM/Ctrl+C, ramki GG6,
parser LOGIN60, CP1250 i port GG32 z jawnym pochodzeniem.

Odniesienie do planu: E00 zakończony wcześniej; E01 zaimplementowany lokalnie.
E02–E12 pozostają do wykonania. Pełny zakres odnotowuje [PARITY.md](PARITY.md).
E08 nie jest upoważnieniem do wdrożenia: obowiązuje późniejsze ustalenie
użytkownika o rozwoju wyłącznie lokalnym do osiągnięcia zgodności.

Uruchomiono kompilację i testy na Linux/WSL z Rust 1.99.1. Test procesu używa
osobnej bazy w katalogu tymczasowym i sprawdza HTTP, drugi proces na tych samych
danych, SIGKILL/restart, nową epokę, migracje, integralność, SIGTERM i odrzucenie
publicznego nasłuchu. Test transportu przechodzi przez prawdziwe TCP oraz granice
protokół → aplikacja → protokół. Nie wykonywano testów klienta natywnego.

Nie wykonano operacji na Containers ani działającym gg-retro. Nie utworzono
zdalnego repozytorium. Autorem lokalnych commitów jest webs1821.

### Najbliższy etap E02

1. Wersjonowane szyfrowanie sekretów i keyring z testami AAD oraz utraty klucza.
2. Porty transakcyjnej rejestracji, zaproszenia, licznik UIN i tombstone.
3. Narzędzie administratora używające tej samej blokady danych.
4. GG_LOGIN60, generacje sesji i minimalny kontrakt prywatności przed `Ready`.
5. Testy niezależnego klienta i wyścigów, następnie osobna kwalifikacja GG6.
