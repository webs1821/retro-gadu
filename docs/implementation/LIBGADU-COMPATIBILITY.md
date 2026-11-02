# Zgodność z niezależną libgadu — dev.31

Test z niezmodyfikowaną libgadu 1.7.0 odtworzył błąd dev.30: zmiana statusu
z opisem bez końcowego NUL rozłączała nadawcę. Odbiorca dostawał stan offline.
Nasze wcześniejsze testy wysyłały zawsze NUL i nie wykrywały tej niezgodności.
Zachowano pakiet syntetycznego opisu i nieudany wynik na poprzedniej binarce.

## Poprawka

Wspólny parser opisów LOGIN60 i NEW_STATUS przyjmuje teraz tekst CP1250 zakończony
granicą pakietu albo NUL. W obu przypadkach obowiązuje limit 70 bajtów. Po NUL
dozwolone są wyłącznie zero lub cztery bajty czasu; pozostałe długości są błędne.
Czterech ostatnich bajtów tekstu bez NUL nie interpretujemy jako czas powrotu.
Zachowanie wyjściowych STATUS60/NOTIFY_REPLY60 i reguły prywatności nie zmieniły się.

To własna implementacja na podstawie zaobserwowanego formatu oraz odczytu API
`gg_login` / `gg_change_status_descr` i nadawania LOGIN60 w źródłach biblioteki.
Nie kopiowano implementacji C do rGG. Rejestr `external-libgadu170-validation`
w THIRD_PARTY.toml wskazuje źródło, autorów i zakres wykorzystania.

## Dowody i granice

Samodzielny tester korzysta z API biblioteki; serwer nie jest z nią linkowany.
Transparentny relay potwierdza rzeczywisty LOGIN60/0x20 i nie modyfikuje bajtów.
Wszystkie konta są syntetyczne, a osobna przestrzeń sieciowa zawiera tylko loopback.
[Recepta i zakres testu](../../validation/libgadu_170/README.md).

Zwykła regresja TCP dodaje logowanie z opisem bez NUL, opisy jedno- i 70-bajtowe
w obu formach, odrzucenie 71 bajtów oraz błędnych długości czasu, dalszą pracę
innych klientów i poprawny czas powrotu. Korpus mutacyjny ma teraz 44 przypadki.
Historyczne raporty wcześniejszych kampanii pozostają niezmienione.

Kolejny zestaw niezależnej libgadu potwierdził katalog WRITE/READ/SEARCH, polskie
filtry, pojedynczy wynik wieku bez pustego wiersza, ActiveOnly i niewidoczność,
usunięcie publikacji oraz trwałość profilu. Kontakty 0/24/2046/2048 B przechodzą
przez API PUT/GET, w tym podział na pakiety, izolację kont i restart.
Dowód: `libgadu-directory-2026-11-02.json`; nowy probe i powtórzenie podstawowego
przepływu są w `libgadu-extended-probe-2026-11-02.json` oraz
`libgadu-extended-core-2026-11-02.json`. Runtime pozostał niezmieniony (dev.31).

`libgadu_media.py` potwierdził także konferencję z dwoma odbiorcami (online/offline
po awarii, jedna kopia dla każdego, poprawny kontekst) oraz klasyczne obrazki.
5120 syntetycznych bajtów przechodzi przez API oferty/request/reply w trzech
fragmentach z dokładnym porównaniem danych i CRC. Obce konto nie dostaje obrazu;
oferta offline przeżywa restart, ale pierwszy odbiór wymaga dostępnego nadawcy,
który może być niewidoczny. Własne formatowanie oferty używa struktur publicznego
API biblioteki, bez kopiowania jej implementacji.

Pięć raportów `libgadu-media-*-2026-11-02.json` / `libgadu-media-2026-11-02.json`
wiąże nowego klienta z niezmienioną biblioteką i runtime dev.31. Pierwsza próba
zakończyła się błędnym oczekiwaniem testu liczby ofert: klucz zawiera message_id,
więc dwa wysłania tworzą dwie oferty. Poprawiono tylko test; pierwszy wynik zachowano.
Przepływy core i katalogu powtórzono na rozszerzonym testerze i przeszły.

Stronicowanie rozszerzono w dev.33: `libgadu_paging.py` publikuje41 profili
przez API libgadu i odbiera strony20/20/1 przez `gg_pubdir50_next` oraz
`GG_PUBDIR50_START`. Kursory23/43/0, brak duplikatów/pustych wpisów, polskie
pola i powtórzenie po restarcie są potwierdzone. W43 logowaniach przechwycono
LOGIN60/0x20 oraz deklarację image_size255. Nowy probe przeszedł również
ponowienie core/katalogu/mediów; biblioteka pozostaje niezmieniona.
Dowody: `libgadu-paging-2026-11-02-dev33.json` i towarzyszące raporty
`libgadu-paging-{probe,core,directory,media}-2026-11-02-dev33.json`.

Pełne bloki kontaktów bez terminatora pozostają poza zakresem tych testów
libgadu. Jej stara funkcja czasu powrotu dopisuje cztery bajty
bez separatora; bez jednoznacznego formatu nie próbujemy odgadywać, gdzie kończy
się tekst. Test klienta GUI GG6 nadal pozostaje odrębną otwartą bramką.

Dev.31 nie zmienia schematu 13 ani zależności runtime. Kwalifikowany obraz i R0
nadal odnoszą się do dev.30. Nie wykonywano push, wdrożenia ani zmian systemowych.
