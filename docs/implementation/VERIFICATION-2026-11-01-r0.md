# Dev.19 — obraz i krótka próba R0

2026-11-01, lokalny WSL, prywatny Docker 29.9.2. Źródło runtime:
`0b251ba42df7197b8fdc07a5855744a70c5c570f`. Kryteria opisano przed próbą
w [R0-LOAD.md](R0-LOAD.md). Kod serwera i progi nie były zmieniane między próbami.

## Wynik

**Krótka próbka R0 zaliczona.** 100 gotowych sesji, 180,001 s, 1800 przyjętych
wiadomości po 128 B: 900 online i 900 offline. Każdy payload sprawdzono;
offline po zalogowaniu potwierdzono także FIFO. Puste listy subskrypcji,
bez transferów obrazków. Gotowość 100 sesji kontrolowano co około sekundę.

| Pomiar | Wynik | Próg |
|---|---:|---:|
| Rzeczywiste nadawanie | 9,9999/s | ≥9,9/s |
| ACK przyjęcia p95 / maksimum | 3,992 / 11,194 ms | ≤250 / <1000 ms |
| Odbiór online p95 / maksimum | 7,621 / 12,155 ms | p95 ≤500 ms |
| Maksymalne spóźnienie generatora | 2,129 ms | ≤250 ms |
| Średnie CPU runtime | 9,71% jednej vCPU | ≤90% |
| Maksymalny RSS | 15,91 MiB | ≤192 MiB |
| Maksymalna pamięć cgroup | 16,02 MiB | limit 256 MiB |
| Swap / OOM | 0 / brak | 0 / brak |

CPU liczono z 178,264 s próbek wewnątrz fazy pomiaru. RSS/HWM obejmuje także
logowanie i końcowy odbiór offline. Twarde cgroup: 1 CPU, 256 MiB, 128 PID,
swap 0. Docker w tym WSL nadal sam nie stosuje memory-swap; sterownik po
sprawdzeniu pełnego ID, etykiety i PID ustawił 0 bezpośrednio w cgroup wyłącznie
swojego kontenera i odczytał wartość zwrotnie.

Po exit 0 sprawdzono stabilną kopię DB+WAL: integrity_check, foreign keys,
brak sesji i payloadów, 1800 rekordów TransportWrittenUnconfirmed. To nie jest
potwierdzenie odbioru oryginalnego GG6; treść odczytał syntetyczny klient testowy.
Własne kontenery/wolumeny usunięto i sprawdzono ich brak; prywatny daemon
zakończył się kodem 0. Nie uruchamiano Docker Desktop ani usług produkcyjnych.

## Dodatkowa regresja i historia przygotowań

22 zestawy kontroli na release binarkach obrazu przeszły w 63,316 s, w tym
cykl życia, konta/kopie/rotacja/hasła/import, GG6, katalog, konferencje/obrazy,
HTTP, metryki i mała kontrola generatora. Ta regresja miała limit RAM/CPU,
ale swap=max; wymagany swap=0 potwierdza oddzielny właściwy pomiar R0.

Zachowano raporty wcześniejszych przygotowań: właściciel pustego wolumenu,
zbyt szybkie zestawianie nowych połączeń i końcowy odczyt WAL na read-only
mount. Żadnego z tych nieukończonych przebiegów nie oznaczono jako sukces R0.
Po poprawkach sterownika wykonano krótki pełny przebieg kontrolny, a następnie
nową pełną próbę 180 s. Szczegóły i brak zmian progów opisuje scenariusz.

## Tożsamość artefaktu

- Runtime: `sha256:f593f3c113b1dece0fd60c05f3df47b621c2733cb29ab5624bd2e149d745e105`.
- Binarka release: `74f509e569400dade1da1fb365520cad115aea989b83fb55516c32c5523e750b`.
- Archiwum źródeł/vendorów: `8a91a28af7aa84ad2ce94cb612e7d03e493f0b3a83330c54a747bc236fc4b85b`.
- Eksport: `rgg-dev19-runtime.tar.gz`, 61 229 424 B,
  SHA256 `ed9c16e3a757d7597c6194e75bfc89215c01733ee555451c642953f775a5906b`.

Komplet z manifestem i raportami:
`D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-container-2026-11-01-dev19`.
W repo: [manifest](container-artifact-2026-11-01-dev19.json),
[regresja obrazu](container-suite-2026-11-01-dev19.json),
[pełny pomiar](r0-2026-11-01-dev19.json),
[krótka kontrola sterownika](r0-2026-11-01-harness-check.json).
Manifest wiąże także hashe obu sterowników, fixture i lockfile. Sterownik
przesłany stdin jest przypisany własnym SHA, oddzielnie od źródła binarki.

## Nadal otwarte

Wynik nie kwalifikuje długiego soak, dużych list kontaktów/obrazków pod
jednoczesnym obciążeniem, klienta natywnego, publicznego profilu ani pełnego
bezpieczeństwa obrazu/OS. Nie ma push ani wdrożenia. Starszy obraz dev.18
i jego raporty pozostają zachowane.
