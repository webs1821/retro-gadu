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

Katalog, obrazki i archiwum kontaktów mają testy rGG, lecz nie są jeszcze objęte
niezależnym klientem libgadu. Jej stara funkcja czasu powrotu dopisuje cztery bajty
bez separatora; bez jednoznacznego formatu nie próbujemy odgadywać, gdzie kończy
się tekst. Test klienta GUI GG6 nadal pozostaje odrębną otwartą bramką.

Dev.31 nie zmienia schematu 13 ani zależności runtime. Kwalifikowany obraz i R0
nadal odnoszą się do dev.30. Nie wykonywano push, wdrożenia ani zmian systemowych.
