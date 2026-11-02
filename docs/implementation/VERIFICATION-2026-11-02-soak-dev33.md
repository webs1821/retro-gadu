# Obraz dev.33 — godzinny test lokalny

Źródło **98354cf99f83315aecc13e8977a4d5fec9d9b26b**, wersja **0.1.0-dev.33**, schema **14**.
Wszystkie bramki [planu zapisanego przed próbą](SOAK.md) przeszły. Runtime
obejmuje m.in. reset hasła administratora offline, poprawkę opisów GG6 oraz
wymaganie prywatnego katalogu danych Unix. Nie zmieniano progów po wyniku.

## Rzeczywisty obraz i przebieg

34 zestawy testów release przeszły w 146,145 s. Osobny test sprawdził trwały
wolumen, restart, wyłączność CLI, backup/verify/restore i działanie po restore.
Następnie wykonano cztery cykle po 900 s nadawania na tym samym procesie i bazie.
Historia narastała od 0 do 36000 wpisów; nie czyszczono jej między cyklami.

Każdy cykl miał 100 sesji Ready, 20 kont offline i tempo 10 wiadomości/s.
Wszystkie **36000** wiadomości zostały przyjęte i dokładnie sprawdzone:
18000 online oraz 18000 offline, bez błędnych payloadów, duplikatów i zmian FIFO.
Między cyklami sprawdzano opróżnienie kolejki, rozłączenie wszystkich klientów
i pięciosekundowy okres bez sesji. Łącznie było 3600 s faz obciążenia; logowanie,
odbiór offline, przerwy i kontrola bazy wydłużyły całą próbę do około 63 minut.

| Cykl | Wiadomości | ACK p95, ms | Online p95, ms | Średnie CPU jednej vCPU |
|---|---:|---:|---:|---:|
| 1 | 9000 | 6.477 | 12.335 | 13.17% |
| 2 | 9000 | 6.950 | 13.168 | 13.65% |
| 3 | 9000 | 7.382 | 13.764 | 14.00% |
| 4 | 9000 | 7.656 | 14.356 | 14.85% |

Najwyższe ACK wyniosło 24.924 ms,
a najwyższy czas odbioru online 55.720 ms.
Pole `cpu_fraction` w JSON oznacza maksimum średnich czterech cykli, a nie średnią
całego testu ani szczyt pojedynczej sekundy. Każdy cykl spełnił osobną bramkę CPU.

## Zasoby, baza i zakończenie

- Rzeczywiste ograniczenia: 1 CPU, 256 MiB RAM, swap0, 128 PID, UID/GID10001,
  root filesystem tylko do odczytu, zero capabilities, no-new-privileges i seccomp.
- 3787 próbek procesu/cgroup; ten sam PID i czas startu.
- Maksymalne RSS **33.82 MiB**; szczyt cgroup
  **47.29 MiB**. Nie było OOM ani użycia swap.
- Mediana RSS bez sesji wzrosła z 20.66 do
  33.82 MiB, czyli o **13.16 MiB** przy progu16MiB.
  Każda przerwa miała trzy próbki. Nie jest to dowód braku wielodniowych wycieków.
- W każdej przerwie było **22 deskryptory i 6 wątków** — brak wzrostu.
- SIGTERM zakończył serwer kodem0. Integralność SQLite i klucze obce były poprawne;
  brak sesji, aktywnych prób dostawy i payloadów; dokładnie36000 wpisów
  `TransportWrittenUnconfirmed`. GG6 nie potwierdza zapisu wiadomości w kliencie.

Docker w WSL początkowo ustawił swap=`max`. Sterownik po sprawdzeniu własności
cgroup wymusił0 i kontrolował go podczas próby; nie zmieniano ustawień hosta.
Kontenery i wolumeny testu usunięto i potwierdzono ich brak. Prywatny daemon
zatrzymano po eksporcie. Obrazy, cache i wcześniejsze archiwa zachowano.

## Archiwum i dowody

- Runtime: `sha256:68fa16274ca92f14af82814cd3fba377517a9c46e2c3d5f334186aa2f8b3d466`.
- Verification: `sha256:d8dd67379d912c09ce98a29e14c6715e6f446a55acf22b311fd0a7c1bcb25054`.
- Serwer release: `267a7f76b33ffe73e707db9d57add571ef592755e6d8dba8948ec4a5025ea620`.
- Źródła w obrazie: `831906cf1c20bd4b2289e7a8259e1398334c8504bc1a6b4292846824c90036ef`.
- `rgg-dev33-runtime.tar.gz`: 62242937 B, SHA256
  `7ec0625c0721e460941a5a6e7a1057c82cc606ac9cde23dfa7691e44b31174cc`.

Archiwum znajduje się w workspace `rgg-container-2026-11-02-dev33`. Sprawdzono
gzip do EOF, manifest, hash konfiguracji i etykietę commitu obrazu.
[Manifest archiwum](container-artifact-2026-11-02-dev33.json) zawiera hashe sześciu
raportów, w tym [pełnych pomiarów](soak-2026-11-02-dev33.json),
[planu przedstartowego](soak-2026-11-02-dev33-plan.json),
[testów release](container-suite-2026-11-02-dev33.json) oraz
[restartu/restore](container-runtime-2026-11-02-dev33.json).

To kwalifikacja lokalna godzinnego ruchu tekstowego, kolejek i ponownych połączeń.
Nie obejmuje natywnego GG6, Windows, WAN, ciągłego ruchu obrazków ani wielu dni
pracy. Pełna kwalifikacja bezpieczeństwa, migracji rzeczywistych danych i wdrożenia
pozostaje otwarta. Nie wykonano push ani wdrożenia.
