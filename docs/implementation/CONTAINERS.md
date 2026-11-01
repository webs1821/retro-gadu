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
pozostają oddzielnymi bramkami. Każdy raport kwalifikuje wskazany commit i obraz,
nie wszystkie późniejsze zmiany źródeł.

## Wynik lokalny

[Raport kwalifikacji](VERIFICATION-2026-11-01-container.md) dokumentuje 20 zestawów
kontroli oraz osobny test PID1, wolumenu, restartu i backup/restore. Limity RAM/CPU
zadziałały, limit swapu w tym WSL nie został zastosowany. Nie ogłasza się profilu R0.

`deploy/compose.local.yaml` odzwierciedla ograniczenia runtime, z `RGG_IMAGE`
wskazującym sprawdzony tag/ID. Sprawdzono jego rozwiniętą konfigurację; nie jest
to profil publicznego dostępu. Przykładowe kontrole artefaktu na lokalnym silniku:

```sh
python3 tests/container_runtime.py --docker /path/to/docker --socket /path/to/local-docker.sock --runtime sha256:RUNTIME_ID --verification sha256:VERIFICATION_ID --report /path/to/new-runtime-report.json
```

Skrypt wymaga dwóch obrazów z tym samym SHA źródła i etykietą lokalnej budowy,
tworzy oznaczone zasoby testowe, a po próbie usuwa wyłącznie swoje kontenery
i wolumeny. `tests/container_suite.py` uruchamia się przez stdin w obrazie
verification, z ograniczeniami wskazanymi w raporcie i tmpfs 128 MiB.


## Kolejny obraz: dev.19

Obraz ze źródła `0b251ba42df7197b8fdc07a5855744a70c5c570f` przeszedł 22 zestawy
regresji i krótką próbkę R0. [Raport](VERIFICATION-2026-11-01-r0.md) i
[manifest](container-artifact-2026-11-01-dev19.json) wiążą obraz, archiwum,
źródła oraz sterowniki testowe. W R0 osobno sprawdzono i wymuszono swap=0
w cgroup własnego kontenera. Nie zmieniano konfiguracji hosta ani Dockera
systemowego; prywatny daemon po tamtych próbach zatrzymano. To nie kwalifikacja
produkcyjna, długi soak ani próba natywnego GG6.

## Obraz dev.22 i wykryty problem dłuższego odtwarzania

Źródło a5d55d37ba29d00b7772629b9d6ce99452a0f1a9 przeszło25 zestawów release
oraz osobny test restartu/wolumenu/backup-restore. Pełny dłuższy R0 (600s)
nie przeszedł: po przyjęciu6000 wiadomości nastąpił timeout przy równoczesnym
odtwarzaniu3000 offline. Nie wydano nowego kwalifikowanego archiwum obrazu.
Raporty container-*-2026-11-01-dev22.json i r0-2026-11-01-dev22-failed.json
zachowują rozdział udanych kontroli funkcji od nieudanej próby obciążenia.
[Poprawka w dev.23](SQLITE-WRITER.md) przeszła osobne odtworzenie, ale nadal
wymaga aktualizacji biblioteki SQLite i nowej pełnej kwalifikacji artefaktu.
