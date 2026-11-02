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
| Ramkowanie GG, fragmentacja, limit, niepełny EOF | gotowe lokalnie; parsery mają deterministyczny test mutacyjny, fuzzing sterowany pokryciem pozostaje otwarty | `frame::tests`, `rgg-transport::tests`, `adversarial.rs`:41 wzorców/11545 prefiksów/92032 bit-flipy/2mln mutacji, limity i zachowanie poprawnych wyników | nie sprawdzono |
| Parser LOGIN60, CP1250, GG32 | gotowe lokalnie | testy protokołu i `gg6_sessions.py` | nie sprawdzono |
| Konta i zaszyfrowane hasła | gotowe lokalnie przez CLI i WWW, offline rotacja kluczy ze wznowieniem; importer ggv1 gotowy na sztucznych danych | accounts::tests, credentials::tests, accounts_cli.py, keys_cli.py | nie dotyczy |
| Zmiana hasła | gotowe lokalnie: offline CLI oraz opcjonalne WWW z pełnym obecnym hasłem, trwałym limitem, barierą sesji i atomowym nonce/wersją/audytem; reset bez starego hasła niezaimplementowany | password_cli.py, http_password.py, password_web_tests: stare/nowe logowanie, lease i dane, kopie, konkurencja, dwie awarie, migracja 11→12 | formularz: obejrzany w przeglądarce; logowanie GG6: testy syntetyczne |
| Logowanie klienta GG | gotowe lokalnie dla LOGIN60/GG32 | logowanie poprawne/błędne, obcy UIN, stare wyzwanie, timeout, pakiet przed loginem | nie sprawdzono |
| Generacje sesji, zastąpienie sesji, reconnect GG po restarcie | sesje i ponowne otwarcie TCP gotowe lokalnie; automatyczny reconnect oryginalnego klienta niepotwierdzony | bariera zamknięcia, stale credentials, stary detach, FIN po SIGTERM | nie sprawdzono |
| Trwała kolejka offline, FIFO, ACK po commit | gotowe lokalnie dla tekstu GG6 | `messaging_tests`, `gg6_sessions.py`, `gg6_concurrent_replay.py`; powtarzany seq, cofnięty zegar,20 odbiorców/3000 offline przy3000 wpisach historii; gg6_history_retention.py:100000 historii/200 offline, nowe przyjęcie, restart i retencja | nie sprawdzono |
| Retry po awarii i uczciwy stan „zapisano do TCP bez potwierdzenia klienta” | gotowe lokalnie; możliwość duplikatu na granicy awarii | cztery kontrolowane awarie procesu, recovery epoki, fencing tokenów | nie sprawdzono |
| Obecność, opis, czas powrotu, niewidoczność, tylko znajomi | gotowe lokalnie; wspólna bramka polityki i publikacji | `presence_tests`, `gg6_presence_contacts.py`: opis, Ready, niewidoczność, blokady, restart | nie sprawdzono |
| Blokowanie i egzekwowanie prywatności przed pierwszą wiadomością | gotowe lokalnie dla NOTIFY FIRST/LAST/EMPTY i ADD/REMOVE | bramka Ready, blokady offline po restarcie, bez wskrzeszania po odblokowaniu; session_contracts.py:42 komendy przed loginem/Ready i w resynchronizacji, bez skutków ubocznych | nie sprawdzono |
| Lista kontaktów, import/eksport, subskrypcje | gotowe lokalnie z poprawionym GG6.1: jawny pusty MORE przy wielokrotności 2047 B, ACK odroczone; oryginał bez poprawki ma nadal niejednoznaczny koniec | 4000 kontaktów, staging, EOF/timeout, 2 awarie przy commit; dev.25: 79 przypadków CPU i 8 przechwyconych sekwencji TCP, restart | funkcja CPU sprawdzona osobno; całe GUI nie sprawdzone |
| Formatowanie wiadomości i konferencje | gotowe lokalnie: style i konferencje do 32 osób; jedna kopia na pakiet klienta | CP1250, bold/italic/underline/RGB, cztery osoby online/offline, brak dodatkowego fan-out, prywatność i awarie | nie sprawdzono |
| Obrazki: oferty, autoryzacja, fragmenty, CRC, dostępny nadawca | gotowe lokalnie; bez trwałego cache, do 255 KiB | image_tests, rgg-transfers, gg6_images.py: TCP/restart, CRC, limity, polityka, sesje, awarie i migracja | nie sprawdzono |
| Katalog: własny profil READ/WRITE | gotowe lokalnie: dobrowolność, zastępowanie, usuwanie i własność | `profile_tests`, `gg6_directory.py` | nie sprawdzono |
| Katalog: filtry, wiek, stronicowanie, brak pustego dodatkowego wiersza | gotowe lokalnie dla GG6; zakres lat zamiast dokładnej daty | polskie filtry, strony 20/20/1, rzadkie pola, prywatność, trwały limit | nie sprawdzono |
| Zaproszenia 3 użycia, numery ręczne/od 112233 | gotowe lokalnie przez CLI i WWW, również adapter starych hashy z zużyciem i active | 12 konkurencyjnych rejestracji, rollback, revocation, restore; accounts_cli.py i legacy_invitations.py | nie dotyczy |
| Rejestracja WWW | gotowe lokalnie, opt-in /register; publiczny profil TLS/proxy niekwalifikowany | http_registration.py: CSRF, limity, 12 konkurencyjnych żądań/3 użycia, GG6 i restart | przeglądarka desktop: wygląd; klient TCP syntetyczny |
| Statystyki online/zarejestrowani z respektowaniem niewidoczności | gotowe lokalnie: strona główna i /api/stats, opt-in | statistics_tests, http_statistics.py; epoka/generacja/credential/Ready, niewidoczność/tylko znajomi, restart/retirement | przeglądarka desktop: wygląd |
| Konfigurowana domena i legacy HTTP discovery | częściowe: opcjonalne lokalne discovery GG6; profil publicznej domeny pozostaje do wykonania | HTTP/1.0, adres listenera, brak wpływu Host, logowanie do odkrytego portu | nie sprawdzono |
| Pobieranie klienta/instalatora, manifest hashy, patcher | katalog WWW/JSON, pakiet CLI i patcher EXE w przeglądarce gotowe lokalnie dla przypiętego GG6.1; bez kwalifikacji wykonania | http_downloads.py, client_package.py: 3 hosty i 464 zasoby Windows; client_downloads.py; client_browser.py: pełna zgodność JS/Python; http_patcher.py i rzeczywiste pobranie w IAB | zasoby/pliki odczytane jako dane; aplikacji GG nie uruchamiano |
| Limity połączeń, żądań, pamięci i kolejki | limity zaimplementowane, także gniazda i czas HTTP; powtarzalna lokalna próba RSS/CPU, bez kwalifikacji produkcyjnej/długiego soak | limity kont/odbiorcy/globalne, TCP per IP, deadline, oversize, wolny odbiorca; http_limits.py, local_endurance.py | nie dotyczy |
| Prywatne metryki operatora | gotowe lokalnie: agregaty bazy, liczniki zdarzeń i histogramy login/accept/delivery na osobnym porcie; dev.27 dodaje CPU/RSS i dostępność próbek; metryki buforów pozostają do wykonania | operations_tests, telemetry tests, http_metrics.py, runtime_metrics.py, process_metrics.py: błędne logowania/ramki, limity, slow reader, restart, cancel i spójność histogramów | nie dotyczy |
| Administrator, rotacja kluczy, backup/restore, importer | częściowe: offline CLI, backup/verify/restore, rotacja oraz pełny import przypiętego gg-retro gotowe lokalnie; brak próby na rzeczywistej kopii | accounts_cli.py, backups_cli.py, keys_cli.py, legacy_import.py: roundtrip, błędne dane, 8 przerwań procesu; migracja 9 → 10 | nie dotyczy |
| Lokalny obraz kontenera, pomiary, soak i kwalifikacja bezpieczeństwa | częściowe: obrazy, krótka próbka R0, restart i restore gotowe lokalnie; długi soak i pełna kwalifikacja nadal do wykonania | dev.26: container_suite.py (27 zestawów), container_runtime.py (restart/restore), container_load.py (100 sesji/6000 wiadomości/600 s, 1 CPU/256 MiB/swap0) przeszły; zachowano nieudaną próbę dev.22 i starsze archiwa. Osobny cgroup ustawia swap0 po weryfikacji własności, bo Docker w WSL go pomija | — |

## Warunek rozważenia podmiany

Obecnie **niespełniony**. Najpierw należy zaimplementować i automatycznie
sprawdzić cały zakres bieżącego gg-retro, a do rzeczywistego wdrożenia także
przejść bramki klientów natywnych, bezpieczeństwa, migracji i odtworzenia z kopii.
Do tego czasu rozwój, testy, dane i procesy pozostają lokalne.

Rozszerzenia takie jak DCC, GG8/10, wielosesyjność nowoczesnych klientów czy
odzyskiwanie haseł nie są automatycznie uznawane za część obecnego gg-retro.
W szczególności jego operacje legacy zmiany/usunięcia konta są wyłączone.
