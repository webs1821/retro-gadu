# Lokalny obraz kontenera — przygotowanie i kwalifikacja

To profil do lokalnych prób. GG nasłuchuje na 127.0.0.1:8074, HTTP na
127.0.0.1:8088 wewnątrz przestrzeni kontenera. Konta i klucze należy zainicjalizować
offline przez rgg-admin w osobnym wolumenie. Nie ma automatycznego tworzenia kont,
importowania danych ani zmiany haseł.

## Budowa z przypiętego źródła

`deploy/Dockerfile` przypina digesty oficjalnych obrazów Rust 1.99.1 i Debian
bookworm-slim oraz osobnego obrazu Python do testów. Runtime ma dwie binarki,
konfigurację, licencję i źródła rGG wraz z vendored Cargo sources w
`/usr/share/rgg/source.tar.gz`. Użytkownik to UID/GID 10001; katalog danych
`/data` ma właściciela 10001 i tryb 0700. Nie ma Python w runtime.

```sh
python3 tools/build_container.py --docker /path/to/docker --socket /path/to/local-docker.sock --target runtime --report /path/to/new-build-report.json
```

Narzędzie wymaga zatwierdzonych zmian i jawnego lokalnego gniazda Unix. Tworzy
kontekst przez `git archive` konkretnego commitu, pomijając nieśledzone ustawienia
i sekrety. Używa pustej konfiguracji klienta Docker i pomija konteksty oraz
zmienne wskazujące inny serwer. Nie uruchamia daemona, nie pushuje obrazu ani nie
uruchamia instancji rGG. Tag zawiera SHA; istniejącego tagu nie nadpisuje.
Raport wskazuje image ID i źródłowy SHA.

Build ma 2 CPU/3 GiB i pobiera zależności w trybie sieci hosta lokalnego silnika.
Po `cargo vendor --locked` kompilacja używa `--frozen`. Runtime jest testowany
osobno, bez sieci zewnętrznej i z mniejszymi limitami. Klasyczny builder umożliwia
lokalną próbę bez osobnego BuildKit; nie jest to konfiguracja zdalnej usługi CI.

## Granice prób

Docelowe ograniczenia uruchomienia: 1 CPU, 256 MiB, 128 PID, user 10001,
read-only root, brak capabilities, `no-new-privileges`, domyślny seccomp,
oddzielny wolumen danych i `--network none`. Testowy klient może współdzielić
wyłącznie przestrzeń sieciową tego kontenera. Nie należy podawać socketu
produkcyjnego Dockera ani stosować host network dla uruchomionej usługi.

Kwalifikacja wykonania, restartu, wolumenu i skutecznych cgroup wymaga osobnego
raportu. Sam Dockerfile i budowa nie są dowodem tych właściwości. Publiczna
domena/reverse proxy, natywny GG6, długi soak i ocena podatności pakietów systemowych
pozostają oddzielnymi bramkami. Runtime Rust pozostaje dev.18.
