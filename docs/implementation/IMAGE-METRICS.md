# Obserwacja buforów obrazków — dev.29

Prywatny endpoint metryk pokazuje stan tego samego koordynatora, którego używają
sesje GG6. Jest to własna implementacja w `rgg-transfers`; port
`ImageObservations` w `rgg-application` oddziela HTTP od konkretnego koordynatora.
Nie ma dodatkowego wątku, samplera, zależności ani migracji bazy.

| Rodzina gauge | Znaczenie |
|---|---|
| `rgg_image_measurements_available` | 1 przy udanej migawce; 0 przy zajętej/uszkodzonej blokadzie lub wyłączonym GG |
| `rgg_image_transfers{state}` | Cztery stałe stany: requested, collecting, ready, missing |
| `rgg_image_reserved_bytes` | Suma zadeklarowanych rozmiarów obrazków w stanach innych niż missing |
| `rgg_image_buffered_bytes` | Suma długości buforów obrazków i nazw plików, które posiada koordynator |
| `rgg_image_buffer_capacity_bytes` | Suma pojemności tych samych buforów; nie obejmuje narzutu alokatora |
| `rgg_image_oldest_transfer_age_seconds` | Monotoniczny wiek najstarszego transferu innego niż missing; 0 przy braku lub przyszłym znaczniku |

Stan requested rezerwuje limit, lecz nie ma jeszcze bufora danych. collecting
alokuje bufor pełnego rozmiaru przy pierwszym fragmencie i zbiera zawartość.
ready oznacza kompletny obrazek po sprawdzeniu CRC. Bufor pozostaje w całości
do oddania ostatniego fragmentu transportowi. missing zachowuje jedynie wpis
potrzebny do powiadomienia odbiorcy, bez bufora ani rezerwacji bajtów; nadal
zajmuje limit liczby wpisów do usunięcia. Migawka nie zawiera identyfikatorów
kont, CRC, nazwy pliku, treści ani opisu.

Pojemność bufora nie oznacza RSS ani pamięci całego procesu. Nie obejmuje
struktury Vec, wpisów transferów, narzutu alokatora, niezależnych kopii fragmentów
oddanych transportowi, ramkowania ani buforów systemowych TCP. Dla całego procesu
służy osobny [pomiar RSS](PROCESS-METRICS.md). Rezerwacja dotyczy danych obrazka,
a bufor obejmuje też nazwę pliku, dlatego te liczby nie muszą być równe.

## Migawka i limity

Odczyt używa `try_lock` i skanuje co najwyżej 32 wpisy. Pod blokadą nie alokuje,
nie wykonuje I/O ani await. Nie czeka na składanie obrazu lub obliczanie CRC.
Zajęta blokada daje availability=0 i pominięcie ośmiu zależnych szeregów w tej
odpowiedzi. Kolejny odczyt może ponownie dać poprawne wartości. Brak pomiaru
nie jest fałszywym zerem i sam w sobie nie oznacza awarii GG.

Migawka jest spójna dla koordynatora, ale nie jest atomowa względem osobnej
migawki bazy czy liczników procesu. Nie korzysta z cache bazy. Odczyt nie
wywołuje wygaszania, nie odświeża deadline i nie usuwa danych. Wygaszenie nadal
następuje przy zwykłych operacjach/pollowaniu koordynatora. W razie opóźnienia
pollowania metryka wieku może przekroczyć 30 s i pokazać stan wymagający obsługi.

Obowiązują istniejące limity: 32 wpisy, 4 na użytkownika, rezerwacja 8 MiB,
255 KiB na obrazek, maksymalnie 512 fragmentów i bezwzględne 30 s od żądania.
Metryki nie zmieniają tych limitów. Rozłączenie odbiorcy usuwa jego wpisy;
rozłączenie nadawcy, błąd, anulowanie lub deadline zwalnia bufory i prowadzi
do missing. Restart usuwa całą pamięć transferów. Trwałe oferty są oddzielne
od buforów i nadal pozwalają ponowić pobranie od zalogowanego nadawcy.

Cały endpoint ma teraz 27 rodzin i najwyżej 126 szeregów, poniżej 16 KiB także
w teście skrajnych wartości wszystkich rodzin. Nie zmieniono osobnego lokalnego
listenera, kontroli Host/Origin ani publicznych statystyk. Prometheusa nie
instalowano. W razie alarmowania należy uwzględnić availability i kilka próbek,
zamiast traktować pojedynczą kolizję blokady jako brak transferów.

## Testy

Testy Rust obejmują wszystkie cztery stany, rezerwację i rzeczywistą pojemność,
pełne 32 alokacje, odrzucenie limitu, CRC/błędną kolejność/overflow, anulowanie,
oba rozłączenia, brak odświeżania i wygaszania przez odczyt, zajętą/uszkodzoną
blokadę oraz format i całkowity rozmiar eksportu. `tests/image_metrics.py`
sprawdza realne TCP/HTTP: odbiór poprawnego obrazka, brak nazwy w metrykach,
restart w trakcie zbierania, ponowne pobranie, limit czterech transferów,
rozłączenia, faktyczny deadline 30 s mimo odczytów/pingów/późnego fragmentu
oraz jawny brak pomiarów w serwerze bez GG.

Kod i testy są własne. Format tekstowy korzysta z już odnotowanych faktów
`prometheus-text-format-reference` w THIRD_PARTY.toml. Nie przeniesiono nowego
kodu z innych projektów. Wyniki kwalifikacji obrazu dev.26 nie obejmują dev.29;
nadal nie wykonano kwalifikacji całego natywnego klienta ani długiego soak.
