# Sesje i dostarczanie wiadomości — dev.3

Zakres tego przyrostu: lokalne LOGIN60/GG32, pojedyncza aktywna sesja konta,
prywatność niezbędna do przesyłania wiadomości i trwały tekst GG6. Konta
przygotowuje istniejące CLI. Opis formatu pochodzi z dokumentacji libgadu;
pochodzenie algorytmu GG32 i kodu testowego wskazuje `THIRD_PARTY.toml`.

## Logowanie i sesje

Transport losuje wyzwanie osobno dla każdego połączenia. Aplikacja otwiera
zaszyfrowany sekret, a adapter porównuje odpowiedź GG32 przez `subtle`.
Nieznany numer wykonuje sprawdzenie z sekretem zastępczym i jest odrzucany;
błędy nie ujawniają, czy konto istnieje. To nie czyni starego GG32 nowoczesnym
protokołem uwierzytelniania: obecny nasłuch pozostaje wyłącznie na loopback.

Rejestr słabych referencji kieruje komendy jednego konta do jednego aktora.
Skrzynka aktora ma 32 miejsca, rejestr najwyżej 128 aktywnych uchwytów.
Aktor nie czeka na innego aktora. Powiadomienie odbiorcy wychodzi z koordynatora
po zatwierdzeniu zapisu; polling co 500 ms odzyskuje pominięte powiadomienia.

Drugie poprawne logowanie żąda zatrzymania starego transportu i czeka na jego
niezależną barierę. Kolejność destrukcji to socket → sygnał zatrzymania → uchwyt
aktora. Dotyczy to również anulowania zadania. Dopiero potem transakcja SQLite
sprawdza `credential_version`, zwiększa `session_generation` i zapisuje nowe ID.
Zastąpienie może wysłać GG_DISCONNECTING; zwykłe zamknięcie serwera kończy TCP
bez tego komunikatu, aby nie blokować automatycznego reconnectu starych klientów.
Reconnect potwierdzono dotąd syntetycznie, nie oryginalnym programem GG.

Każdy skutek komendy sprawdza w tej samej transakcji konto, wersję sekretu,
generację, ID sesji i epokę procesu. Stary detach nie usuwa nowej sesji.
Port unieważnienia wersji poświadczeń również najpierw zatrzymuje transport;
operacyjny reset hasła i rotacja kluczy pozostają osobnymi zadaniami.

## Gotowość i prywatność

Po LOGIN_OK klient nie jest jeszcze `Ready`. Komplet FIRST…LAST, LAST lub EMPTY
atomowo zastępuje reguły kontaktów. Złożenie listy ma stały deadline; kolejne
FIRST go nie przedłużają. Maksimum to 400 pozycji na pakiet i 4000 na listę.
Niepełna lista, timeout albo rozłączenie pozostawiają poprzednie blokady w bazie.

ADD sumuje flagi bitowo, REMOVE usuwa wskazane bity. BLOCKED odrzuca wiadomości
od danego nadawcy oraz kończy oczekujące dostarczenia bez zachowania treści.
Odblokowanie nie wskrzesza tych wiadomości. Flaga friends-only jest zapisana
dla przyszłej obsługi obecności i nie jest traktowana jako blokada wiadomości.
Publikowanie obecności nie jest częścią tego etapu: odpowiedź na subskrypcje
zawiera stan offline, także dla zalogowanych kontaktów.

Socket ma jednego writera i najwyżej jeden aktywny lease. Zmiana prywatności
w tej sesji jest przetwarzana, gdy writer jest wolny. Bajtów już zapisanych
nie można odwołać. Zewnętrzne zastąpienie/unieważnienie sesji zamyka socket
przed zmianą autoryzacji. Bezpośredni port repozytorium wymaga tej samej bariery;
nie wolno go używać jako skrótu omijającego aktora w przyszłym panelu administracyjnym.

## Trwałość, ACK i FIFO

Jedna transakcja zapisuje wiadomość oraz dostarczenie `Pending`. Dopiero po
commit nadawca dostaje ACK=3 (przyjęto do kolejki), również dla odbiorcy online.
Nie wysyłamy ACK=2 oznaczającego dostarczenie do użytkownika. Flaga klasy 0x20
wyłącza ACK nadawcy. Przekazana przez nadawcę flaga „queued” jest ignorowana;
adapter ustala ją z sesji odbiorcy. Brak konta i blokada dają wspólny wynik 6,
przekroczony limit kolejki daje 4.

Kolejność wyznacza rosnący `enqueue_order`, niezależnie od czasu zegara i seq
klienta. Powtórzone seq nie są deduplikowane: stary klient może je legalnie
ponownie użyć. Treść jest normalizowana z CP1250 do UTF-8, style mają strukturę
niezależną od protokołu, a payload ma wersję 1. Tekst ma limit 2000 bajtów CP1250,
style — 256 pozycji. Konferencje, obrazy i nieobsługiwane opcje są odrzucane przed
zapisem, bez pozornego potwierdzenia sukcesu.

Claim przypisuje `Leased` do losowego tokenu, konkretnej sesji, generacji i epoki.
Zegar monotoniczny ogranicza rzeczywisty zapis (domyślnie 2 s, liczone od początku
żądania claim). `lease_until` w UTC jest informacją techniczną; samo przekroczenie
tej daty nie pozwala drugiemu writerowi przejąć wysyłki. Ponowienie jest możliwe
po dowodzie zamknięcia transportu albo po nowej epoce procesu.

Pełny zapis do TCP przechodzi warunkowo do `TransportWrittenUnconfirmed`.
**GG6 nie przesyła potwierdzenia odbiorcy.** Nie ma gwarancji exactly-once:
awaria po zapisie do socketu, przed zapisem wyniku w bazie, może dać duplikat.
Z kolei pełny zapis zaakceptowany przez system operacyjny nie dowodzi odebrania
przez aplikację klienta; późniejsza utrata po stronie sieci/klienta nie jest
wykrywalna tym protokołem. Nie przedstawiamy tej sytuacji jako potwierdzonego odczytu.

Po terminalnym wyniku treść i style są usuwane logicznie z rekordów. Nie jest
to gwarancja fizycznego usunięcia bajtów z WAL, wolnych stron bazy ani kopii.
Treści oczekujących wiadomości nie są szyfrowane przez moduł sekretów kont.

## Recovery, limity i obsługa błędów

Wyłączna blokada systemowa katalogu dowodzi zakończenia poprzedniego procesu.
Przed uruchomieniem listenerów nowa epoka zamienia stare Leased w Pending,
zamyka techniczne próby jako RecoveredEpoch i usuwa stare sesje. Reguły prywatności
pozostają. Odbiorca ponownie przechodzi synchronizację przed odbiorem kolejki.
Nieprawidłowy zapisany JSON payloadu daje terminalny błąd integralności zamiast
nieskończonej ponownej próby tego samego rekordu.

Domyślne limity konfiguracji:

| Zasób | Limit |
|---|---|
| Aktywne połączenia / z jednego IP | 64 / 16 |
| Nowe połączenia z jednego IP | burst 64, odnowienie 16/s |
| Pakiety w jednej sesji | burst 400, odnowienie 100/s |
| Logowanie / początkowa lista / bezczynność | 5 s / 5 s / 90 s |
| Oczekujące wiadomości na odbiorcę / nadawcę / globalnie | 1000 / 1000 / 10 000 |
| Zapisany payload na odbiorcę / globalnie | 4 MiB / 32 MiB |
| TTL wiadomości Pending | 30 dni |
| Terminalna historia diagnostyczna | do 7 dni, najwyżej 100 000 rekordów przy sprzątaniu |
| Historia prób jednej wiadomości | ostatnie 32 próby |

Wolny odbiorca ma własny deadline, ograniczony bufor TCP i nie blokuje aktora
nadawcy. Wygaszanie nie odbiera prawa aktywnemu writerowi Leased. Sprzątanie
działa cyklicznie co 30 s oraz przy operacjach kolejki. Limity payloadu nie są
limitem fizycznego rozmiaru pliku SQLite; nie przeprowadzono jeszcze pomiarów
RSS, długiego soak ani kwalifikacji dysku i obciążenia dla wydania.

Zwykły build nie zawiera mechanizmów wymuszania awarii. Opcja `test-hooks`
uruchamia je wyłącznie dla syntetycznego seq `0xfeed`; skrypt testowy tworzy
własny proces, katalog i porty. Testowane granice: przed commit przyjęcia,
po commit przed ACK, po claim przed zapisem oraz po zapisie przed utrwaleniem wyniku.

## Rozszerzenie dev.9

Schemat 7 dodaje uporządkowany kontekst konferencji, zapisany w tej samej
transakcji co treść i dostarczenie. JSON uczestników wchodzi do limitów bajtów.
Usunięcie treści terminalnej czyści także kontekst. Migracja starych wiadomości
dodaje pustą listę i koryguje liczbę bajtów o jej dwa znaki.
[Pełny kontrakt konferencji](CONFERENCES.md).
