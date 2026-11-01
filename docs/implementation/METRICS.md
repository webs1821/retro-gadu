# Prywatne metryki operacyjne

Wprowadzono w dev.19, rozszerzono w dev.22. Funkcja jest domyślnie wyłączona. Opcjonalne pole
`metrics_bind = "127.0.0.1:9090"` na najwyższym poziomie konfiguracji włącza
osobny listener. Akceptowane są wyłącznie adresy loopback, także IPv6;
nie można współdzielić tego samego endpointu z HTTP/GG. Port 0 wybiera port
testowy, ogłaszany w lokalnym logu. Przykład użycia: GET
`http://127.0.0.1:9090/metrics` z dokładnie takim Host.

To interfejs operatora dla lokalnych procesów, bez uwierzytelnienia. Nie należy
udostępniać go przez publiczny reverse proxy. Liczby obejmują także sesje
niewidoczne i „tylko znajomi”. Publiczne `/api/stats` zachowuje dotychczasową
politykę widoczności i nie eksportuje tych metryk. Router strony WWW nie ma
trasy `/metrics`, a router metryk nie zawiera strony ani operacji kont.

## Kontrakt danych

Własna implementacja na podstawie dokumentacji
[formatu Prometheus Text 0.0.4](https://prometheus.io/docs/instrumenting/exposition_formats/)
i [konwencji nazw](https://prometheus.io/docs/practices/naming/), sprawdzonych
2026-11-01. Nie kopiowano kodu biblioteki eksportera. Odpowiedź UTF-8 z końcowym
LF, `text/plain; version=0.0.4; charset=utf-8`, stałe HELP/TYPE. Poniższy zakres
obejmuje podstawowe 12 rodzin/19 gauge. Dev.22 dodaje liczniki i histogramy
[runtime](RUNTIME-METRICS.md): łącznie 17 rodzin/112 szeregów, poniżej 16 KiB.
Brak etykiet pochodzących od użytkowników.

| Rodzina | Znaczenie |
|---|---|
| `rgg_build_info{version}` | Wersja kompilacji, wartość 1 |
| `rgg_uptime_seconds` | Monotoniczny czas od przygotowania listenera metryk |
| `rgg_database_schema_version` | Wersja schematu |
| `rgg_snapshot_timestamp_seconds` | Czas UTC migawki bazy |
| `rgg_accounts{state}` | Istniejące konta active/disabled |
| `rgg_retired_numbers` | Zachowane rezerwacje po usuniętych kontach |
| `rgg_sessions{phase}` | Bieżące sesje ready/synchronizing po sprawdzeniu epoki, generacji, credential_version i aktywności konta; Ready także wiąże obecność z sesją |
| `rgg_delivery_records{state}` | Zachowane rekordy pending/leased/written_unconfirmed/expired/privacy_rejected/rejected |
| `rgg_queued_payload_bytes` | Logiczny rozmiar payloadów Pending + Leased, nie rozmiar SQLite ani RSS |
| `rgg_oldest_queue_age_seconds` | Wiek najstarszego Pending/Leased, zero przy pustej kolejce lub czasie z przyszłości |
| `rgg_contact_archive_bytes` | Bajty UTF-8 zatwierdzonych archiwów kontaktów |
| `rgg_unexpired_image_grants` | Oferty przed terminem ważności; aktualna prywatność i sesje nadal warunkują użycie |

Rekordy terminalne są usuwane przez retencję, więc ich liczby mogą maleć.
`written_unconfirmed` oznacza zapis do TCP, nie potwierdzenie odbioru GG6.
Pending/Leased obejmuje wszystkie fizycznie zachowane stany aż do sprzątania,
również rekordy oczekujące na obsługę wygaszenia. Migawka nie zmienia danych.

## Granice i błędy

Agregaty powstają w jednej transakcji odczytu SQLite. Zapytania zwracają liczby
i sześć znanych kategorii stanów; nie pobierają treści ani identyfikatorów kont.
Cache współdzieli wynik maksymalnie przez 500 ms od rozpoczęcia odczytu.
Tylko jeden odczyt bazy może trwać równocześnie; dwa żądania mogą czekać lub
korzystać z wyniku. Całość odczytu/oczekiwania ma deadline 1 s.

Brak Ready, błąd bazy lub timeout daje 503 bez częściowego/starego wyniku.
Przekroczenie dwóch obsługiwanych scrape daje 429. Wymagany jest dokładny Host,
Origin (jeśli obecny) musi być zgodny, a Sec-Fetch-Site pozwala tylko na
same-origin/none. Odrzucane są powielone nagłówki tych pól, query, request body
i Transfer-Encoding. Nagłówki przekazywane przez proxy nie zmieniają polityki.
Brak CORS; wszystkie odpowiedzi mają no-store i nosniff.

Listener ma niezależne limity: 8 gniazd globalnie, 4/IP, 5 s bezczynności,
5 s zablokowanego zapisu i 60 s całkowitego życia połączenia. Jego awaria
uruchamia wspólne zamknięcie usług; shutdown wyłącza Ready i kończy listener.

Dev.22 obejmuje histogramy logowania/przyjmowania/dostarczania i skończony
zestaw wyników runtime. Nadal brak metryk buforów transferów/RSS/CPU w endpointcie.
Zewnętrzny lokalny pomiar
procesu pozostaje w `tests/local_endurance.py` i `tests/container_load.py`.
Krótka próba R0 na obrazie dev.19 jest opisana w
[osobnym raporcie](VERIFICATION-2026-11-01-r0.md). Długi soak pozostaje otwarty.
Nie uruchamiano Prometheusa.
