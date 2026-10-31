# Zgodność funkcjonalna rGG z gg-retro

Punkt odniesienia: `webs1821/gg-retro` **0.8.3**, commit
`ed9640cc247764754b5e47891053eb07a53f76d5`. Aktualizacja: 2026-10-31.

Statusy: **gotowe lokalnie** = kod i wskazane testy; **częściowe** = brak całego
przepływu; **brak** = do implementacji. „Natywny” oznacza test konkretnej wersji
oryginalnego GG, a nie test Python/Rust. W żadnym wierszu nie dziedziczymy
wyniku testów działającego gg-retro.

| Funkcja / wymaganie | Implementacja rGG | Dowód automatyczny | Natywny GG |
|---|---|---|---|
| Izolacja modułów i rejestr pochodzenia | gotowe lokalnie | `tools/verify_repository.py` | nie dotyczy |
| Konfiguracja lokalna, błędy opcji, live/ready | gotowe lokalnie | testy `config`, `tests/lifecycle.py` | nie dotyczy |
| OS lock, SQLite WAL/FULL, migracje, nowa epoka po awarii | gotowe lokalnie | testy store + `lifecycle.py` | nie dotyczy |
| Ramkowanie GG, fragmentacja, limit, niepełny EOF | gotowe lokalnie | `frame::tests`, `rgg-transport::tests` | nie sprawdzono |
| Parser LOGIN60, CP1250, GG32 | częściowe: parser i algorytm | testy `gg6`, `legacy_hash`; syntetyczne wektory | nie sprawdzono |
| Konta, zaszyfrowane hasła, bezpieczne logowanie | brak | — | — |
| Generacje sesji, zastąpienie sesji, reconnect GG po restarcie | brak | — | — |
| Trwała kolejka offline, FIFO, ACK po commit | brak | — | — |
| Retry po awarii i uczciwy stan „zapisano do TCP bez potwierdzenia klienta” | brak | — | — |
| Obecność, opis, czas powrotu, niewidoczność, tylko znajomi | brak | — | — |
| Blokowanie i egzekwowanie prywatności przed pierwszą wiadomością | brak | — | — |
| Lista kontaktów, import/eksport, subskrypcje | brak; otwarta bramka zakończenia uploadu 2047 B | — | — |
| Formatowanie wiadomości i konferencje | brak | — | — |
| Obrazki: oferty, autoryzacja, fragmenty, CRC, dostępny nadawca | brak | — | — |
| Katalog: własny profil READ/WRITE | brak | — | — |
| Katalog: filtry, wiek, stronicowanie, brak pustego dodatkowego wiersza | brak | — | — |
| Rejestracja WWW, zaproszenia 3 użycia, numery ręczne/od 112233 | brak | — | — |
| Statystyki online/zarejestrowani z respektowaniem niewidoczności | brak | — | — |
| Konfigurowana domena i legacy HTTP discovery | brak | — | — |
| Pobieranie klienta/instalatora, manifest hashy, patcher | brak w nowym projekcie | — | — |
| Limity połączeń, żądań, pamięci i kolejki | częściowe: granica ramki; reszta brak | test oversize | — |
| Administrator, rotacja kluczy, backup/restore, importer | brak | — | — |
| Lokalny obraz kontenera, pomiary, soak i kwalifikacja bezpieczeństwa | brak | — | — |

## Warunek rozważenia podmiany

Obecnie **niespełniony**. Najpierw należy zaimplementować i automatycznie
sprawdzić cały zakres bieżącego gg-retro, a do rzeczywistego wdrożenia także
przejść bramki klientów natywnych, bezpieczeństwa, migracji i odtworzenia z kopii.
Do tego czasu rozwój, testy, dane i procesy pozostają lokalne.

Rozszerzenia takie jak DCC, GG8/10, wielosesyjność nowoczesnych klientów czy
odzyskiwanie haseł nie są automatycznie uznawane za część obecnego gg-retro.
W szczególności jego operacje legacy zmiany/usunięcia konta są wyłączone.
