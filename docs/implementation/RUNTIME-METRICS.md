# Liczniki i histogramy runtime — dev.22

Realizacja części M22 planu, ukończona i sprawdzona lokalnie. Nowe metryki są
prywatne na istniejącym listenerze; bez publicznego portu, trwałego stanu ani
nowych zależności. Baza pozostaje schema12. [Weryfikacja](VERIFICATION-2026-11-01-dev22.md).

Stałe kategorie kompilowane w kodzie, bez etykiet UIN/IP/wiadomości. Jeden
mały snapshot pod mutexem obejmuje liczniki, aktywne gniazda i histogramy.
Brak I/O i await w sekcji krytycznej. Arytmetyka saturująca, stała alokacja.
Zakres życia operacji liczy anulowanie także przy porzuceniu future.

- Login: od początku negocjacji do zapisu LOGIN_OK lub rozpoznanej odmowy.
- Accept: parsowanie zwykłej wiadomości/bundle i obserwowany wynik aktora;
  bez zapisu ACK do sieci i bez image-control. Success oznacza obserwowany commit.
- Delivery: od rozpoczęcia claim do zapisu ramki i zarejestrowania transportu
  w bazie. To nadal brak potwierdzenia odbiorcy. Ukończone puste poll są pomijane;
  timeout, błąd lub anulowanie claim liczymy także bez poznania jego wyniku.
- Te liczniki resetują się przy restarcie i nie są księgą trwałych zdarzeń.
  Anulowane oczekiwanie na aktora może pozostawić zatwierdzoną operację.
- Histogramy: <=1/5/10/50/100/500 ms, 1/5/15 s i +Inf, wspólny count/sum.
  Czas monotoniczny, bez wieku wiadomości offline i bez pomiaru zegarem UTC.

## Eksport i spójność

| Rodzina | Znaczenie |
|---|---|
| `rgg_gg_connections` | Sockety przechowywane przez rozpoczęte handlery GG, także przed logowaniem; bez HTTP i bez końcowego sprzątania aktora po zamknięciu socketu |
| `rgg_gg_admissions_total{result}` | accepted, address_capacity, rate, global_limit, ip_limit; pierwszy powód odmowy według istniejącej kolejności kontroli |
| `rgg_gg_connection_ends_total{reason}` | finished, shutdown, replaced, peer_closed, protocol, timeout, storage, rate, io, stale, other, cancelled |
| `rgg_operations_total{operation,result}` | login/accept/delivery oraz jeden z 13 stałych wyników; zdarzenie obserwowane przez transport |
| `rgg_operation_duration_seconds{operation}` | Histogram: osobne `_bucket{le}`, `_sum` i `_count`, dla tych samych zakończonych obserwacji |

Wyniki operacji: success, rejected, quota, invalid, timeout, transport, stale,
busy, storage, integrity, not_ready, cancelled, other. Wszystkie kombinacje
są eksportowane, również zerowe. Wynik rejected logowania odzwierciedla decyzję
Authenticator; nie rozróżnia nieznanego konta, błędnego hasła i szczegółów
odmowy. Success logowania oznacza zapis LOGIN_OK, nie zakończenie synchronizacji.

Łącznie z agregatami DB: **17 rodzin, 112 szeregów**, odpowiedź poniżej 16 KiB.
Format wynika z [oficjalnej specyfikacji](https://prometheus.io/docs/instrumenting/exposition_formats/).
Własna implementacja bez pakietu eksportera; pochodzenie odnotowane w THIRD_PARTY.
Nieużywane kategorie nie tworzą nowych szeregów, nie ma dynamicznej mapy etykiet.

Snapshot runtime jest spójny: suma wyników danego rodzaju równa `_count`,
kumulacyjne kubełki rosną, +Inf równa `_count`. Po nasyceniu u64 cała agregacja
tej operacji zatrzymuje się, żeby nie zawinąć wartości ani rozjechać sum.
Suma czasu ma saturujące u128 nanosekund, eksportowane jako sekundy; przy bardzo
dużych liczbach reprezentacja float ma zwykłe ograniczenie dokładności.
Snapshot ma mniej niż 2 KiB, bez nieograniczonych kolekcji.

Formatowanie odbywa się po zwolnieniu mutexa. Agregaty DB mogą pochodzić z cache
do 500 ms; runtime jest kopiowany przy każdym scrape. Nie są jednym atomowym
snapshotem DB/procesu i nie należy traktować różnicy ich czasów jako utraty danych.
Przy braku Ready albo błędzie snapshotu DB nadal zwracamy 503, bez częściowego
eksportu. Ograniczenia listenera i żądań pozostają takie jak w [METRICS](METRICS.md).

## Co sprawdzono i czego metryki nie dowodzą

Testy obejmują granice kubełków i overflow, równoległe snapshoty, porzucenie
zadania z rzeczywistym socketem i sygnałem fence, błędne ramki i logowanie,
oba timeouty logowania/synchronizacji, wymianę sesji, tekst, brak odbiorcy,
quota i parser, offline/restart, limity IP/globalne/napływu oraz wolnego odbiorcę.
Ten ostatni przypadek sprawdza count timeout, zerowy pozostały lease i
Interrupted→Pending, nie tylko obecność innych wiadomości w kolejce.

Licznik może zniknąć z restartem, także pomiędzy commit a ACK. Anulowanie
oczekiwania nie dowodzi rollback operacji aktora. Zapis TCP i oznaczenie w DB
nie są potwierdzeniem klienta GG6. Nadal możliwy duplikat na granicy awarii.
Brak metryk RSS/CPU, rozmiarów buforów, pełnego śledzenia każdej odmowy image-control
czy histogramu shutdown. Kategorie Other i Cancelled pozostają jawne.
Nie uruchamiano Prometheusa ani promtool. Nie wykonano nowego obrazu/R0 dev.22.
