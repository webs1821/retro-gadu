# Pełna historia dostarczeń — dev.26

## Odtworzony problem

Na dev.25 przygotowano nową, syntetyczną bazę ze 100000 zakończonych dostarczeń
i 200 wiadomościami offline dla 20 odbiorców. Przez rzeczywiste TCP odebrano
109 wiadomości; następnie część połączeń zakończyła się EOF. Pozostałe 91
wiadomości zachowało stan Pending. Integralność i klucze obce były poprawne.
Raport nieudanego przebiegu zachowano jako history-diagnostic-2026-11-01-dev25.json.
Nie ma w nim szczegółowego kodu przyczyny zamknięcia poszczególnych połączeń.

Poprzednie sprzątanie tworzyło listy identyfikatorów ze wszystkich terminalnych
dostarczeń lub wszystkich wiadomości. Było wywoływane również podczas obsługi
pojedynczych wiadomości i zmian prywatności. Koszt rósł więc wraz z historią,
chociaż jej treść została już wyczyszczona.

## Zmiana

Migracja 13 dodaje indeks częściowy messages_with_payload. Zawiera wyłącznie
wiadomości z niepustym polem body, styles lub conference. Czyszczenie rozpoczyna
się od tych rekordów, a stan dostarczenia sprawdza po kluczu głównym. Nie buduje
listy wszystkich zakończonych dostarczeń.

Wygasanie Pending i odrzucanie przez prywatność korzystają z istniejącego indeksu
aktywnej kolejki deliveries_fifo. Jawny warunek Pending/Leased pozwala zastosować
ten indeks; dodatkowy warunek Pending nadal wyklucza żywego właściciela Leased.
Sprawdzenie wiadomości i reguł prywatności jest powiązane z konkretnym rekordem.
To te same predykaty biznesowe, bez zmiany limitów, terminów lub izolacji transakcji.

Przy okazji poprawiono usuwanie konta: usuwa również conference, obok body/styles,
w tej samej transakcji. Wcześniej ten fragment metadanych czekał na osobne
sprzątanie kolejki. Istniejący test usunięcia konta sprawdza teraz wszystkie pola.

Retencja pozostaje siedmiodniowa, z przycinaniem do 100000 najnowszych rekordów
według terminal_at i message_id. Jak wcześniej, przejścia do stanu terminalnego
mogą chwilowo przekroczyć tę liczbę przed następnym prune podczas przyjęcia,
claim lub okresowego utrzymania. Nie zmieniono tego wprowadzając indeks.

## Walidacja

Ten sam scenariusz diagnostyczny na kandydacie dev.26 odebrał 200/200 w 2,937 s,
bez EOF. Raport history-diagnostic-2026-11-01-dev26.json zachowuje wynik osobno.
To kontrola funkcjonalna w WSL, bez limitów zasobów cgroup i bez kwalifikacji R0.

Nowy tests/gg6_history_retention.py w CI i zestawie kontenera sprawdza:

- 100000 wpisów historii, 200 offline, 20 równoczesnych odbiorców, payload i FIFO;
- brak timeoutów dostarczeń w metrykach, nowe przyjęcie i odbiór offline;
- wyczyszczenie wszystkich trzech pól payloadu oraz brak niedokończonych prób;
- restart i przycięcie historii do dokładnie 100000 właściwych rekordów;
- integralność bazy i brak pozostawionych sesji.

Test magazynu dodatkowo zasila ponad 100000 rekordów, także ze szczątkowym
styles/conference bez body. Sprawdza czyszczenie, granicę wieku i remis czasu,
zachowanie aktywnego lease przy skoku UTC, wygaśnięcie Pending i brak wskrzeszenia
wiadomości po cofnięciu zegara. Nie zwiększono limitów czasu testów serwera.

tests/sqlite_upgrade.py odtwarza kopię z prawdziwych binarek dev.25 i otwiera ją
dev.26. Wymaga --expected-schema 13, zachowania poprzednich 12 sum migracji,
obecności nowego indeksu i prawidłowego działania kont, offline, obrazków,
kontaktów, katalogu oraz nowego backup/restore. Oryginał i stara kopia niezmienione.
Stary admin służy do odtworzenia starej kopii; nie wykonuje się downgrade bazy 13.

Wyniki zbiorcze: history-regression-2026-11-01-dev26.json i
history-faults-2026-11-01-dev26.json. Ostatni kwalifikowany obraz/R0 pozostaje
dev.24. Nowa wersja wymaga osobnej kwalifikacji obrazu. Żadnych zmian produkcji,
publikacji pakietów klienta ani push.
