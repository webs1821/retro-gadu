# Retro Gadu (rGG)

Nowy serwer protokołu Gadu-Gadu, pisany w Rust. Punktem odniesienia jest fork
`webs1821/gg-retro` 0.8.3. Rozwój odbywa się **wyłącznie lokalnie**, do czasu
osiągnięcia co najmniej jego zakresu funkcjonalnego. Plan nie stanowi dowodu
gotowej implementacji, a testy syntetyczne nie oznaczają kwalifikacji klienta GG.

## Aktualny stan

Pierwszy działający etap infrastruktury: osobne moduły, konfiguracja,
blokada katalogu danych na poziomie systemu operacyjnego, SQLite WAL/FULL,
migracje, identyfikator uruchomienia, HTTP live/ready i kontrolowane zamknięcie.
Są również fundamenty kodeka GG6, parser logowania i port GG32.

Drugi przyrost dodaje konta obsługiwane przez lokalne CLI: szyfrowane hasła,
keyring, transakcyjne zaproszenia i przydział numerów, trwałą rezerwację numerów
usuniętych kont oraz testy równoczesnych rejestracji.

**Serwer nie obsługuje jeszcze logowania klienta ani rozmów GG.** Nie uruchamia publicznego
nasłuchu GG. TCP i przejście pakiet → komenda → zdarzenie → pakiet są obecnie
sprawdzane w odizolowanym teście integracyjnym.

- [Szczegółowy plan](PLAN_IMPLEMENTACJI.md)
- [Macierz funkcjonalności i braków](docs/implementation/PARITY.md)
- [Dziennik implementacji](docs/implementation/STATUS.md)
- [Wyniki weryfikacji](docs/implementation/VERIFICATION-2026-10-31.md)
- [Pochodzenie użytych elementów](THIRD_PARTY.toml)
- [Recenzje planu](docs/reviews/ROZLICZENIE_UWAG.md)

## Uruchomienie lokalne

Wymagania: Rust wskazany w `rust-toolchain.toml`, kompilator C dla SQLite oraz
Python 3.11+ do kontroli repozytorium. Bieżącą implementację uruchomiono i
przetestowano na Linuksie w WSL; natywnego buildu Windows jeszcze nie kwalifikowano.

```sh
cargo build --workspace --locked
cargo run --locked -p rgg-server -- --config config/rgg.example.toml
```

Sprawdzenie: `http://127.0.0.1:8088/health/ready`. Zakończenie: Ctrl+C lub SIGTERM.
Konfiguracja odrzuca adresy inne niż loopback i nieznane klucze.
Ścieżka danych jest względna względem pliku konfiguracji, nie katalogu uruchomienia.

Baza i lock są tworzone w podanym katalogu danych. Nie usuwaj `rgg.lock`, gdy
proces działa. Blokadę zwalnia zamknięcie uchwytu procesu, również po awarii.
W WSL zalecany jest katalog danych w linuksowym systemie plików, np.
`/tmp/rgg-dev-data` dla danych jednorazowych. Nie używaj NFS/SMB ani produkcyjnej bazy.

## Weryfikacja

```sh
python tools/verify_repository.py
python tools/gg32_vectors.py
cargo metadata --locked --format-version 1 | python tools/dependency_inventory.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python tests/lifecycle.py --binary target/debug/rgg-server
python tests/accounts_cli.py --admin target/debug/rgg-admin --server target/debug/rgg-server
cargo audit
```

Testy `lifecycle.py` i `accounts_cli.py` wymagają Linuksa/WSL i używają wyłącznie katalogu tymczasowego,
syntetycznej bazy i losowego portu loopback. Jeśli ustawiono `CARGO_TARGET_DIR`,
ścieżkę do pliku wykonywalnego należy odpowiednio zmienić.
Do audytu użyto `cargo-audit 0.22.2`, instalowanego przez
`cargo install cargo-audit --version 0.22.2 --locked`.

Workflow CI jest przygotowany jako plik lokalny. Nie został uruchomiony na GitHubie;
repozytorium nie ma jeszcze zdalnego adresu.

## Lokalne konta i zaproszenia

Zatrzymaj proces rGG przed pracą narzędziem administratora. Oba programy
używają tej samej blokady katalogu; CLI odmawia dostępu, gdy działa serwer.
W Linux/WSL można przygotować jednorazowe dane testowe tak:

```sh
target/debug/rgg-admin init --data-dir /tmp/rgg-dev-data
target/debug/rgg-admin invite --data-dir /tmp/rgg-dev-data --count 2 --uses 3
python tools/create_account.py --admin target/debug/rgg-admin --data-dir /tmp/rgg-dev-data
target/debug/rgg-admin stats --data-dir /tmp/rgg-dev-data
```

Aby serwer używał tych samych danych, ustaw w jego konfiguracji
`data_dir = "/tmp/rgg-dev-data"`. Ten katalog służy wyłącznie jednorazowym próbom.

Interaktywny pomocnik przekazuje hasło i kod przez standardowe wejście,
bez umieszczania ich w argumentach procesu. Bezpośredni `account-create` przyjmuje
JSON na stdin: `requested_uin` (liczba albo null), `invite`, `password`, `email`.
Hasło na tym etapie ma 8–15 drukowalnych znaków ASCII bez spacji, zgodnie z polityką
dotychczasowego gg-retro. Adres e-mail zapisuje się jako **niezweryfikowany**.

Puste pole numeru oznacza pierwszy wolny numer od 112233. Numer ręczny może być
niższy; nie przesuwa automatycznego licznika do wysokiej wartości. Zaproszenie
jest zużywane dopiero wraz z zatwierdzeniem utworzenia konta. Kolizja numeru,
błąd szyfrowania i nieudana rejestracja nie zużywają zaproszenia.
Obecne CLI tworzy nowe kody rGG; importer kont i zaproszeń z gg-retro pozostaje do wykonania.

`retire --uin <numer> --data-dir <katalog>` usuwa rekord konta i trwale rezerwuje
numer. Nie jest to obietnica usunięcia jego dawnych danych z kopii ani śladów
w plikach systemowych. Nie ma jeszcze interfejsu WWW ani zdalnego administratora.

Hasła są przechowywane w kopertach XChaCha20-Poly1305 z losowym nonce; powiązanie
z numerem, wersją i identyfikatorem klucza chroni przed podmienianiem rekordów.
Baza utrzymuje historię użytych par klucz–nonce także po usunięciu konta i odrzuca kolizję.
`keys.toml` musi być zachowany razem z informacją, do której bazy należy.
Na Linuksie program wymaga dostępu do pliku kluczy wyłącznie dla właściciela;
w WSL używaj linuksowego systemu plików. Brak lub niewłaściwy klucz dla istniejących
kont jest błędem, a nie powodem do wygenerowania zamiennika. Automatyczna rotacja,
kopie i odzyskiwanie pozostają kolejnymi zadaniami.

## Podział kodu

| Moduł | Odpowiedzialność |
|---|---|
| `rgg-domain` | Identyfikatory i czyste reguły danych |
| `rgg-application` | Komendy, zdarzenia, przypadki użycia i porty infrastruktury |
| `rgg-protocol` | Czyste dekodery/enkodery i reguły formatu GG |
| `rgg-store-sqlite` | Implementacja portów, migracje, trwałość i wyłączne posiadanie danych |
| `rgg-transport` | Adapter strumienia TCP do ograniczonych ramek |
| `rgg-http` | Endpointy HTTP zależne od portów aplikacji |
| `rgg-server` | Konfiguracja, składanie modułów, sygnały i cykl życia |
| `rgg-credentials` | Koperty sekretów, keyring, losowe zaproszenia i ich skróty |
| `rgg-admin` | Lokalne operacje na kontach i zaproszeniach pod blokadą danych |

```mermaid
flowchart TD
    Server[rgg-server] --> HTTP[rgg-http]
    Server --> Store[rgg-store-sqlite]
    HTTP --> App[rgg-application]
    Store --> App
    Transport[rgg-transport] --> Protocol[rgg-protocol]
    Protocol --> App
    Protocol --> Domain[rgg-domain]
    App --> Domain
```

Własny kod zabrania `unsafe`. Kontrola grafu zależności pilnuje m.in., aby SQLx i
Axum nie trafiały do domeny, aplikacji ani parsera. Kolejne moduły powstają wraz
z realną implementacją, zamiast pustych pakietów na zapas.

## Autorstwo i źródła

Własny kod: **webs1821**, AGPL-3.0-or-later. Pełna licencja w [LICENSE](LICENSE).
Funkcja GG32 została przeniesiona z poggadaj (Oreeeee); zachowano informację
o autorze, licencję, oryginalny plik, commit i jego sumę SHA-256. Pythonowy
generator wektorów również jest portem tego algorytmu, a nie niezależnym klientem.

Opis protokołu libgadu służy jako dokumentacja formatu, a gg-retro jako odniesienie
funkcjonalne. Kod biblioteki libgadu ani serwera gg-retro nie został skopiowany.
Każde przyszłe wykorzystanie fragmentu kodu wymaga odrębnego wpisu w
`THIRD_PARTY.toml`, zachowania licencji i wskazania konkretnych symboli.
Oryginalny gg-retro ma niestandardową licencję MIS, nie MIT.

Wersje zewnętrznych bibliotek Rust są przypięte w `Cargo.lock`. Ich własne
licencje pozostają w mocy; rejestr pochodzenia kodu źródłowego rGG nie zastępuje
inwentaryzacji tych zależności ani przyszłego audytu wydania.

Inwentaryzacja wersji, licencji deklarowanych w metadanych i sum źródeł znajduje się
w [dependencies.json](docs/implementation/dependencies.json), a wynik RustSec w
[raporcie audytu](docs/implementation/cargo-audit-2026-10-31.json).
Warstwa bazy używa bezpośrednio zgodnych wersji `sqlx-core` i `sqlx-sqlite`;
nie rozwiązuje zbędnych sterowników MySQL/PostgreSQL ani ich zależności.
Migracje są osadzone w programie i zachowują kontrolę sum przez migrator SQLx.
