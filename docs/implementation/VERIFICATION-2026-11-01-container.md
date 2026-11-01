# Lokalny kontener rGG dev.18 — 2026-11-01

Artefakt runtime z commitu `7bc2d669a6039b0c770186d781edbc537dce2f9e`, schema 11.
Wersja aplikacji pozostaje dev.18; przyrost dotyczy pakowania i kwalifikacji.
Obraz i testy uruchomiono tylko na komputerze użytkownika, w osobnym silniku WSL.
Nie było push, publikacji obrazu ani połączenia z produkcyjnym Containers.

## Artefakt i pochodzenie

- Runtime image ID: `sha256:b0f5d9eae97ba7d4c12e2e068730e48ebf3e7a75ed7a0ad3fa84fca0102673ab`.
- Verification image ID: `sha256:cf3a7b38b9bb927ad7a83dfef102f8c4cb498e3bdbf7cb54ad1bc54a7d995ffe`.
- Release binarka rgg-server: `ecc6b15263b9363014bd69fa46cae839907cf30c9d97ee031bcb1c4ed5b529c0`.
- Archiwum odpowiadających źródeł wewnątrz obrazu:
  `d276e1f994acbdfb33aa8af193353d81748165a2ef5ce3563a19a4025d851a99`.
- Runtime 118090595 B; zapisany plik tar.gz 61115802 B, SHA-256
  `d4cc60eba8bff1445c4ade5ed7bae43810daf5a0897194e5a04eff3c79b770fa`.

Bazy są przypięte digestami w Dockerfile. Źródłem kontekstu jest wyłącznie
`git archive` wskazanego SHA. Dwa testy tego kontraktu przeszły na Windows i WSL.
Obsłużono stary Git 2.25.1 przez jawne ścieżki git-dir/work-tree; nie zmieniono
globalnej konfiguracji użytkownika. Źródła rGG i wszystkich vendored zależności,
wraz z ich licencjami, są w `/usr/share/rgg/source.tar.gz`. Kompilacja po vendoringu
używa `--frozen`. Nie deklaruje się bitowej powtarzalności całego obrazu.

## Rzeczywiście przeprowadzone próby

20 zestawów kontroli na release binarkach obrazu zakończyło się poprawnie w
48,558 s. Obejmują pochodzenie, parser pakietu klienta, inspekcję/import starej
bazy, lifecycle, konta, klucze, hasła, backup/restore, GG6, obecność/kontakty,
katalog, konferencje, obrazki, stare zaproszenia oraz wszystkie funkcje HTTP.
To zwykły build bez test-hooks. Kontrolowane SIGKILL/restart należące do normalnych
testów procesu zostały wykonane; nie powtarzano wariantów wymagających test-hooks.

Osobno sprawdzono obraz runtime jako PID1: UID/GID 10001, brak capabilities,
seccomp i no-new-privileges, read-only root, brak sieci i publikowanych portów.
Własny wolumen został zainicjalizowany przez rgg-admin. Potwierdzono odmowę CLI
przy działającym serwerze, odebranie wiadomości offline po restarcie kontenera,
kompletną kopię, jej weryfikację i odtworzenie do nowego wolumenu. Nowa instancja
logowała konta i przyjmowała wiadomości. Zamknięcia SIGTERM były poprawne,
bez OOM. Testowe kontenery i wolumeny usunięto i niezależnie sprawdzono ich brak.

Zestawy danych były wyłącznie syntetyczne. Obrazy nie zawierają tych wolumenów,
produkcyjnych danych ani binarek starego klienta GG. Compose został rozwinięty
i sprawdzony poleceniem `config`; wykonanie testów odbywało się przez jawne CLI,
nie przez Compose. Nie uruchomiono Docker Desktop.

## Limity i otwarte bramki

W cgroup sprawdzono `memory.max=268435456`, `cpu.max=100000 100000`, `pids.max=128`.
Natomiast mimo zażądanego `--memory-swap 256m`, skuteczne `memory.swap.max` miało
wartość `max`. WSL ma aktywny swap; silnik ostrzegł o braku obsługi jego limitu.
Sam fakt istnienia interfejsu swap w raporcie suite **nie oznacza** zastosowania
limitu. Próba potwierdza limit RAM, lecz nie wspólny pułap RAM+swap.

Nie kwalifikowano R0 (100 sesji/10 wiadomości na sekundę), wielogodzinnego soak,
natywnego GG6/Windows, domeny publicznej i TLS/reverse proxy ani podatności
pakietów systemowych obrazów. RustSec pozostaje wynikiem lockfile dev.18;
nie jest skanem pakietów Debiana. Do wdrożenia nadal nie ma kwalifikacji.

## Środowisko i pliki

Tymczasowy silnik miał prywatny socket, nowy data-root, wyłączony mostek oraz
zmiany iptables/forwardingu. Przed kontenerami zastąpiono w tym katalogu stary
zestaw narzędzi oficjalnym [Docker 29.9.2](https://docs.docker.com/engine/release-notes/29/),
z runc 1.5.2. Zestaw statyczny służy wyłącznie tej próbie, zgodnie z
[przeznaczeniem opisanym przez Docker](https://docs.docker.com/engine/install/binaries/).
Systemowych programów nie wymieniano. Po eksporcie silnik zakończono poprawnie.

Archiwum: `D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-container-2026-11-01\rgg-dev18-runtime.tar.gz`.
Raporty: `container-build-2026-11-01-dev18.json`,
`container-test-image-2026-11-01-dev18.json`, `container-suite-2026-11-01-dev18.json`,
`container-runtime-2026-11-01-dev18.json`. Hashy sterowników testów nie zastępuje
SHA źródła aplikacji: sterowniki wykonano osobno na wskazanym artefakcie.
