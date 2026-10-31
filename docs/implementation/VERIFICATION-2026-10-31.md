# Weryfikacja lokalna — 2026-10-31

Wersja: `0.1.0-dev.2`. Zakres: E01 i część kont/sekretów E02a.
Środowisko: Linux w WSL na komputerze użytkownika, Rust 1.99.1
(`58cf8f9c3`, 2026-10-13), cargo-audit 0.22.2; kontrola TOML/inwentaryzacji
przez lokalny Python 3.13. Katalog artefaktów testowych: `/tmp/rgg-target`.

## Wyniki

| Kontrola | Wynik |
|---|---|
| `cargo test --workspace --locked` | 28 zaliczonych, 0 błędów, 0 pominiętych |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | zaliczone |
| `cargo fmt --all -- --check` | zaliczone |
| `cargo build --workspace --locked` | zaliczone |
| `tests/lifecycle.py` | zaliczone: HTTP, lock, crash/restart, epoka, migracje, integralność, SIGTERM, odmowa publicznego bind |
| `tests/accounts_cli.py` | zaliczone: konta, numery, limity zaproszeń, retirement, brak plaintext w DB, brak/wadliwy klucz, wykluczenie CLI/serwera |
| `tools/verify_repository.py` | pochodzenie, 9 modułów, licencje własnych plików, komplet osadzonych migracji |
| `tools/gg32_vectors.py` | 6 syntetycznych wektorów; nie jest to dowód interoperacyjności |
| `tools/dependency_inventory.py` | 189 zewnętrznych wersji bibliotek, wszystkie mają metadane licencji |
| `cargo audit --json` | 0 znanych podatności, 0 ostrzeżeń, bez wyjątków ignorujących zgłoszenia |
| Oryginalny plan i recenzje | sumy SHA-256 niezmienione |
| Dotychczasowy lokalny gg-retro | czysty, HEAD `ed9640cc247764754b5e47891053eb07a53f76d5` |

Testy kont obejmują 12 konkurencyjnych rejestracji do zaproszenia o trzech
użyciach, równoczesną rezerwację jednego UIN, rollback błędu szyfrowania,
kolizję nonce po usunięciu konta oraz migrację istniejącego zaszyfrowanego konta
ze schematu 2 do schematu 3. Są to wyłącznie dane syntetyczne w katalogach tymczasowych.

## Audyt zależności

Końcowy [raport JSON](cargo-audit-2026-10-31.json) używa bazy RustSec:
`2295466c4e9df26647fd67d8fa1c705ac2fc7f81`, aktualizacja 2026-10-30,
1395 wpisów. Cargo.lock obejmuje 198 pakietów, w tym 9 własnych modułów.
SHA-256 lockfile: `172bfa5ebf7e8c4a98339de856816205cc6fe5e13dfe3f0fd8c15d82e6c16edd`.

Pierwszy przebieg wskazał RUSTSEC-2023-0071 w opcjonalnym `rsa` pochodzącym
z rozwiązywania dodatkowych sterowników SQLx. `cargo tree -i rsa` nie wskazał
aktywnej ścieżki w sprawdzanym buildzie Linuksa. Ograniczono zależności do
`sqlx-core` i `sqlx-sqlite`, usuwając RSA również z lockfile. Zachowano migrator
SQLx i jego kontrolę sum; źródło migracji jest osadzone przez `include_str!`.
Końcowy wynik nie korzysta z ignorowania tego zgłoszenia.

Audyt dotyczy znanych zgłoszeń dla przypiętych wersji. Nie jest dowodem
kompletnej poprawności całego serwera ani ręcznym audytem bibliotek kryptograficznych.

## Granice potwierdzenia

Nie ma jeszcze nasłuchu obsługującego logowanie GG ani wiadomości. Nie wykonano
testów prawdziwych GG 6.0/6.1, kwalifikacji obciążeniowej, importu produkcyjnej
bazy, operacyjnej rotacji kluczy ani odtwarzania serwera z backupu.
Workflow CI jest zapisany, lecz nie był uruchamiany na zdalnym GitHubie.
Nie utworzono zdalnego repozytorium i nie wykonywano działań na Containers.
Pełny stan funkcji wskazuje [macierz zgodności](PARITY.md).
