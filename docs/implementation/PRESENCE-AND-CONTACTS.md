# Obecność i kontakty — dev.4

Implementacja E04 jest lokalna. Źródła formatu: `libgadu-wire-reference`,
`libgadu-legacy-reference` i `gg-retro-compatibility-reference` z THIRD_PARTY.toml.
Parsery i logika transakcji są własną implementacją; nie kopiowano kodu tych serwerów.

## Obecność zależna od obserwatora

LOGIN60 ustawia oczekiwany status. Dopiero kompletna lista prywatności atomowo
zatwierdza reguły, obecność i Ready. Zmiana statusu przed Ready nie publikuje danych.
Obsługiwane są dostępny, zajęty, niewidoczny i niedostępny, opis Windows-1250 do
70 bajtów oraz opcjonalny czas powrotu. Od dev.31 wejściowy opis może kończyć się
granicą pakietu lub NUL. Czas jest rozpoznawany wyłącznie jako cztery bajty po NUL;
nie zgadujemy go z końcówki tekstu. [Test niezależnej libgadu](LIBGADU-COMPATIBILITY.md). STATUS60 i NOTIFY_REPLY60 mają osobne
enkodery: tylko snapshot ma bajt długości opisu. Metadane DCC pozostają zerowe.

Polityka `effective_presence_for` ukrywa cały opis i czas, kiedy nadawca jest
niewidoczny, któraś strona blokuje drugą, flaga kontaktu nakazuje pokazywanie
offline albo tryb tylko znajomi nie pozwala obserwatorowi na wgląd. Bit BUDDY=1
oznacza subskrypcję; FRIEND=2 jest uprawnieniem, a BLOCKED=4 blokadą.
Po odblokowaniu nowy stan pojawia się w ramach kolejnej publikacji.

Opisy jawnie ustawionego statusu offline są trwałe i podlegają tym samym
regułom widoczności. Nagłe zerwanie połączenia nie utrwala opisu online ani
niewidocznego. Start nowej sesji usuwa poprzedni opis offline przed Ready.

Wspólna bramka publikacji obejmuje odczyt widoczności aż do końca zapisu do
socketu. Mutacja prywatności czeka na tę bramkę przed commit. Odczyt nie czeka
na komendę aktora obserwatora, więc nie tworzy cyklu blokad. Jeden deadline
zapisu ogranicza cały snapshot, również oczekiwanie na bramkę i bazę. Bariera
zamknięcia poprzedniego socketu poprzedza przejęcie sesji i bramki mutacji.
Przyszły administrator online musi zachować ten sam porządek.

Transport scala kolejne zmiany i odpyta rewizję co 500 ms. Cache do 4000 kontaktów
przechowuje tylko ostatni stan widoczny temu odbiorcy. Snapshot jest dzielony na
pakiety po 128 rekordów; aktualizacje wysyłają tylko różnice. To nie jest dziennik
wszystkich chwilowych statusów. Test obciążenia długotrwałego pozostaje otwarty.

## Archiwum listy kontaktów i granica 2047 bajtów

Archiwum GG_USERLIST jest osobnym, wersjonowanym dokumentem. Zachowuje CP1250,
kolejność i zakończenia wierszy; nie zastępuje reguł prywatności ani subskrypcji.
Nie interpretuje jeszcze poszczególnych kolumn rodzimego formatu listy GG6.
Limit to 262144 znaków CP1250 / 786432 bajty UTF-8, 4000 wierszy, bez NUL.
Łączny budżet opublikowanych i przygotowywanych dokumentów wynosi 96 MiB.

PUT zaczyna nowy staging powiązany z sesją, epoką, generacją i bazową wersją.
MORE dopisuje do tego stagingu. Eksport w trakcie uploadu nadal zwraca poprzednią
pełną wersję. Zastąpienie sesji, EOF, timeout i restart usuwają tylko staging.
Transakcja publikuje nową wersję w całości; ACK sukcesu wychodzi dopiero po commit.

**Profil uploadu pozostaje ograniczony.** W legacy GG_USERLIST blok ma najwyżej
2047 bajtów. Typ MORE nie mówi, czy jest ostatni. Libgadu wysyła ostatni pełny
blok bez osobnego terminatora, jeżeli rozmiar jest wielokrotnością 2047. Nie ma
wówczas dowodu końca dokumentu. rGG publikuje dopiero po fragmencie krótszym niż
2047, również po pustym MORE. Sam pełny blok nigdy nie zastępuje starej listy.
Po domyślnie 10 s (`network.archive_seconds`, zakres 1–60) sesja jest zamykana,
staging porzucany, a poprzednia wersja pozostaje dostępna.

Potwierdzenia wszystkich fragmentów są odroczone do publikacji (maks. 130).
Przegląd przypiętego libgadu 9eb2aa0 (2026-11-01) potwierdził, że funkcja
gg_userlist_request wysyła wszystkie fragmenty przed oczekiwaniem na odpowiedzi.
Nie daje to jednak dowodu zachowania zamkniętego GG6.
Klient wysyłający kolejne fragmenty dopiero po ACK może wymagać innego,
osobno udowodnionego profilu. Nie wymyślamy nieudokumentowanych kodów błędów.
Eksport nie ma tej niejednoznaczności: końcowy pakiet ma odrębny typ GET_REPLY.

Testy sprawdzają długości 0, 1, 2046, 2047, 2048, 4094, 4095, opóźniony pełny
fragment, EOF, timeout, nowy PUT, restart i awarie przed/po commit. Bramka
kwalifikacji oryginalnego GG6 pozostaje otwarta; nie deklarujemy pełnego pokrycia
importu wszystkich rzeczywistych klientów. To jawnie ograniczony wariant
bezpiecznego zapisu dopuszczony przez plan, a nie domniemane rozwiązanie
niejednoznacznego końca uploadu.
