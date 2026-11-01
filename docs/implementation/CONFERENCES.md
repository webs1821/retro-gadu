# Konferencje GG6 — dev.9

Implementacja niezależna według [opisu libgadu](https://libgadu.github.io/protocol/),
sekcja konferencji; wpis `libgadu-wire-reference` w THIRD_PARTY.toml. Kod C nie
był portowany. GG6 wymaga osobnego testu natywnego; obecne dowody są syntetyczne.

## Jedna kopia na adresata

GG_SEND_MSG ma jednego głównego odbiorcę. Opcja 0x01 niesie numery pozostałych
uczestników; klient wysyła osobny pakiet do każdego odbiorcy. rGG nie rozsyła
ponownie według tej listy. Czteroosobowa rozmowa A/B/C/D daje trzy pakiety od A
i trzy dostarczenia, nie dziewięć. Ten sam seq w różnych kopiach jest poprawny;
nie deduplikujemy wiadomości na podstawie seq.

Kontekst jest deklaracją nadawcy, nie pokojem zarządzanym przez serwer ani
dowodem członkostwa innych kont. Numery kontekstu nie muszą istnieć; nie służą
do autoryzacji, pobierania danych, statusów czy obrazków. Rzeczywista tożsamość
nadawcy zawsze pochodzi z zalogowanej sesji. Prywatność i aktywność sprawdzamy
dla głównego odbiorcy każdego pakietu. Zablokowana kopia nie dociera, pozostałe
mogą zostać przyjęte; klient dostaje niezależne ACK dla adresatów.

## Granice formatu

- Maksymalnie 32 uczestników: nadawca, adresat i do 30 innych numerów.
  Liczba sprawdzana przed alokacją. Ramka wiadomości nadal ma limit 4096 B.
- Kolejność numerów zachowana. Duplikaty, zero, główny adresat, uwierzytelniony
  nadawca lub numer poza zakresem GG6 są odrzucane. Zwykła wiadomość do siebie
  pozostaje dozwolona; konferencja kierowana do siebie jest niepoprawna.
- Opcje konferencji i formatowania można podać w dowolnej kolejności, każdą
  najwyżej raz. Wyjście ma kolejność konferencja → style. Opcja z zerową liczbą
  dodatkowych osób jest normalizowana do zwykłej rozmowy. Nieznane opcje,
  obcięcia i dane obrazków nadal odmawiają; bez ACK sukcesu.
- CP1250, bold/italic/underline/RGB oraz znaczenie flag ACK/queued są zachowane.
  Pusty tekst i sterowanie obrazkami pozostają poza tym przyrostem.

## Trwałość i migracja

`ConferenceContext` jest osobnym typem aplikacji, z prywatną listą i walidacją.
Jego Debug pokazuje tylko liczebność. `MessageBundle` i `DeliveryLease` przenoszą
ten typ. Schemat 7 dodaje `messages.conference`: uporządkowany JSON numerów,
do 512 B. Wchodzi do limitów bajtów kolejki i tej samej transakcji co tekst,
style i dostarczenie. ACK=3 dopiero po commit. Awaria przed commit nie zostawia
częściowego kontekstu; po commit jest on odzyskiwany razem z wiadomością.

Migracja starych wiadomości dodaje pustą listę i 2 B do rozliczenia aktywnej
treści. Nie wymyśla uczestników ani nie zmienia stylów. Starszy serwer odmówi
otwarcia schematu 7; ewentualny rollback wymaga odtworzenia całej kopii v6.
Zmiana bajtów może chwilowo przekroczyć ustawioną kwotę już pełnej starej kolejki:
nowe wiadomości są odrzucane z wynikiem pełnej kolejki do zwolnienia miejsca;
wcześniej przyjęte nie są przez to usuwane.

Przy odczycie sprawdzamy semantykę JSON, tożsamości, głównego adresata i wersję
payload. Uszkodzony rekord jest terminalnie odrzucony i czyszczony. Usunięcie
treści po zakończeniu dostarczenia, blokadzie lub wygaśnięciu usuwa również
kontekst. Oferta obrazka będzie wymagała osobnego modelu retencji w E06b.

ACK=3 i zapis do TCP nadal nie dowodzą odbioru przez GG6; awaria między TCP
a zapisem wyniku może powtórzyć kopię. Konferencja nie zmienia tego kontraktu.
Nie obiecujemy atomowego sukcesu wszystkich pakietów wysłanych przez klienta.

## Dowody

`tests/gg6_conferences.py`: cztery osoby, offline/restart, style i porządek,
blokady każdego adresata, błędne opcje, brak podwójnego fan-out, no-ACK, 32 osoby,
kwoty bajtów i awarie. `--baseline-dir` pozwala sprawdzić bazę utworzoną przez
binarki wcześniejszego dev.8 (schemat 6). [Raport dev.9](VERIFICATION-2026-11-01-dev9.md).
