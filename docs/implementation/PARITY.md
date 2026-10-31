# Zgodność funkcjonalna rGG z gg-retro

Punkt odniesienia: `webs1821/gg-retro` **0.8.3**, commit
`ed9640cc247764754b5e47891053eb07a53f76d5`. Aktualizacja: 2026-11-01.

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
| Parser LOGIN60, CP1250, GG32 | gotowe lokalnie | testy protokołu i `gg6_sessions.py` | nie sprawdzono |
| Konta i zaszyfrowane hasła | gotowe lokalnie przez CLI; rotacja operacyjna i importer brak | `accounts::tests`, `rgg-credentials::tests`, `accounts_cli.py` | nie dotyczy |
| Logowanie klienta GG | gotowe lokalnie dla LOGIN60/GG32 | logowanie poprawne/błędne, obcy UIN, stare wyzwanie, timeout, pakiet przed loginem | nie sprawdzono |
| Generacje sesji, zastąpienie sesji, reconnect GG po restarcie | sesje i ponowne otwarcie TCP gotowe lokalnie; automatyczny reconnect oryginalnego klienta niepotwierdzony | bariera zamknięcia, stale credentials, stary detach, FIN po SIGTERM | nie sprawdzono |
| Trwała kolejka offline, FIFO, ACK po commit | gotowe lokalnie dla tekstu GG6 | `messaging_tests`, `gg6_sessions.py`, powtarzany seq i cofnięty zegar | nie sprawdzono |
| Retry po awarii i uczciwy stan „zapisano do TCP bez potwierdzenia klienta” | gotowe lokalnie; możliwość duplikatu na granicy awarii | cztery kontrolowane awarie procesu, recovery epoki, fencing tokenów | nie sprawdzono |
| Obecność, opis, czas powrotu, niewidoczność, tylko znajomi | gotowe lokalnie; wspólna bramka polityki i publikacji | `presence_tests`, `gg6_presence_contacts.py`: opis, Ready, niewidoczność, blokady, restart | nie sprawdzono |
| Blokowanie i egzekwowanie prywatności przed pierwszą wiadomością | gotowe lokalnie dla NOTIFY FIRST/LAST/EMPTY i ADD/REMOVE | bramka Ready, blokady offline po restarcie, bez wskrzeszania po odblokowaniu | nie sprawdzono |
| Lista kontaktów, import/eksport, subskrypcje | częściowe: subskrypcje i archiwum gotowe lokalnie; upload wymaga końcowego fragmentu krótszego niż 2047 B, ACK odroczone | 4000 kontaktów, granice fragmentów, staging, EOF/timeout, 2 awarie przy commit | nie sprawdzono |
| Formatowanie wiadomości i konferencje | podstawowe style tekstu gotowe lokalnie; konferencje odrzucane | CP1250, bold/italic/underline/RGB, nieobsługiwane opcje bez ACK sukcesu | nie sprawdzono |
| Obrazki: oferty, autoryzacja, fragmenty, CRC, dostępny nadawca | brak | — | — |
| Katalog: własny profil READ/WRITE | gotowe lokalnie: dobrowolność, zastępowanie, usuwanie i własność | `profile_tests`, `gg6_directory.py` | nie sprawdzono |
| Katalog: filtry, wiek, stronicowanie, brak pustego dodatkowego wiersza | gotowe lokalnie dla GG6; zakres lat zamiast dokładnej daty | polskie filtry, strony 20/20/1, rzadkie pola, prywatność, trwały limit | nie sprawdzono |
| Zaproszenia 3 użycia, numery ręczne/od 112233 | gotowe lokalnie przez CLI | test 12 konkurencyjnych rejestracji, kolizji numeru i rollback; `accounts_cli.py` | nie dotyczy |
| Rejestracja WWW | brak | — | — |
| Statystyki online/zarejestrowani z respektowaniem niewidoczności | brak | — | — |
| Konfigurowana domena i legacy HTTP discovery | brak | — | — |
| Pobieranie klienta/instalatora, manifest hashy, patcher | brak w nowym projekcie | — | — |
| Limity połączeń, żądań, pamięci i kolejki | limity zaimplementowane; brak kwalifikacji wydajności/RSS/soak | limity kont/odbiorcy/globalne, TCP per IP, deadline, oversize, wolny odbiorca | nie dotyczy |
| Administrator, rotacja kluczy, backup/restore, importer | częściowe: offline CLI i walidacja kluczy; reszta brak | `accounts_cli.py` | nie dotyczy |
| Lokalny obraz kontenera, pomiary, soak i kwalifikacja bezpieczeństwa | brak | — | — |

## Warunek rozważenia podmiany

Obecnie **niespełniony**. Najpierw należy zaimplementować i automatycznie
sprawdzić cały zakres bieżącego gg-retro, a do rzeczywistego wdrożenia także
przejść bramki klientów natywnych, bezpieczeństwa, migracji i odtworzenia z kopii.
Do tego czasu rozwój, testy, dane i procesy pozostają lokalne.

Rozszerzenia takie jak DCC, GG8/10, wielosesyjność nowoczesnych klientów czy
odzyskiwanie haseł nie są automatycznie uznawane za część obecnego gg-retro.
W szczególności jego operacje legacy zmiany/usunięcia konta są wyłączone.
