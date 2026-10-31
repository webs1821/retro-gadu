# Katalog publiczny GG6 — dev.5

Dobrowolny profil jest związany z uwierzytelnionym kontem. Sama rejestracja nie
umieszcza konta w wynikach, także przy wyszukiwaniu po numerze. WRITE zastępuje
cały formularz; pominięte pola są czyszczone. Pusty formularz, również z samą
płcią, usuwa profil. Numer właściciela pochodzi wyłącznie z sesji. READ nie jest
interfejsem do czytania cudzych prywatnych pól profilu.

## Format i pochodzenie

Adapter jest niezależną implementacją formatu opisanego przez
[libgadu](https://libgadu.github.io/protocol/) (`libgadu-wire-reference`).
Fakty zgodności rodzimego GG 6.1.0.158 pochodzą z dokumentu
`docs/gg6-directory-compatibility.md` we wcześniejszym forku gg-retro **0.8.3**,
commit `ed9640cc247764754b5e47891053eb07a53f76d5`
(`gg-retro-compatibility-reference`). Nie przenoszono jego kodu, testów ani binariów.

| Operacja | Żądanie | Sukces | Odmowa |
|---|---|---|---|
| WRITE | 1 | 1 | 2 |
| READ | 2 | 3 | 4 |
| SEARCH | 3 | 5 | 6 |

READ/WRITE odsyłają dowolną sekwencję, w tym zero. SEARCH wymaga niezerowej.
SEARCH nie emituje pustych wartości tekstowych ani nieznanego roku: skaner
GG6 uznaje każdą parę NUL za granicę osoby. READ używa innego parsera klienta
i zachowuje puste pola formularza. Mapowanie płci w READ/WRITE jest odwrotne
niż w filtrze SEARCH. Wszystkie te przypadki mają testy pakietów.

Obserwacje starego forka dotyczyły klienta o SHA-256
`1db383709d5ba22f52f21f049697cbc70865db57f034489a2b774cf23944469d`.
Nie uruchamiano go przeciw rGG. Nie dziedziczymy jego wcześniejszej kwalifikacji;
GG 6.0 oraz rzeczywisty GG 6.1 pozostają do osobnego sprawdzenia.

## Wyszukiwanie i prywatność

Filtry obejmują numer, imię, nazwisko lub nazwisko panieńskie, pseudonim,
miejscowość lub miejscowość pochodzenia, płeć, pojedynczy rok / zakres lat
urodzenia i widoczną aktywność. Łączenie filtrów oznacza AND. Tekst jest
dopasowywany jako fragment bez rozróżniania wielkości liter, z jawnym
uproszczeniem ą/ć/ę/ł/ń/ó/ś/ź/ż do a/c/e/l/n/o/s/z/z. Znaki `%`, `_` i apostrof
są zwykłym tekstem, nie wildcardem ani częścią SQL. To nie jest pełny Unicode
casefold; granica GG6 używa Windows-1250.

GG6 przesyła rok lub przedział lat, nie dokładną datę urodzenia. Serwer nie
wylicza dokładnego wieku co do dnia. Przyjmowane lata: od 1900 do bieżącego
roku UTC, granice zakresu włącznie. Brak roku nie pasuje do filtra roku.

Wyniki zawierają numer, imię, pseudonim, miejscowość, znany rok i efektywny
status. Nie zawierają nazwiska, płci, pól rodzinnych, e-maila, hasła ani opisu
statusu. Dobrowolnie opublikowany profil pozostaje publiczny również offline.
Niewidoczność, blokada w dowolnym kierunku, „zawsze offline” i tylko znajomi
ukrywają obecność dokładnie jak na liście kontaktów; filtr ActiveOnly używa
tej widoczności, a nie samego istnienia sesji. Reguły współdzielą funkcję
`effective_presence_for` i dekodowanie stanu z bazy.

Odczyt i wysyłka wyników trzymają wspólne pozwolenie publikacji. Zmiana
prywatności i usunięcie profilu czekają na zakończenie rozpoczętego zapisu.
Żadna odpowiedź nie czeka na aktora z trzymanym pozwoleniem odczytu.

## Limity i trwałość

Migracja 0006: public_profiles i directory_rate. Zmiana profilu jest atomowa,
sprawdza sesję, epokę, generację i wersję poświadczeń. Stare sesje nie zapisują.
Usunięcie konta usuwa profil i jego stan limitowania przez klucze obce.

Strona mieści 20 osób, uporządkowanych po numerze. `nextstart` to pierwszy
pasujący numer niewysłany na obecnej stronie; kolejna strona zaczyna od niego
włącznie. `fmstart` i `start` są aliasami. Nie ma powtórzeń wynikających ze zmiany
offsetu. Strony są odczytywane z aktualnej bazy, bez zamrożonego snapshotu całego
wyszukiwania — legacy protokół nie przekazuje tokenu takiej sesji wyszukiwania.

Maksymalnie 10000 profili; po 60 znaków w polu, do 4096 B żądania i 16384 B
odpowiedzi. Odczyt strumieniuje kandydatów zamiast ładować cały katalog do RAM.
Termin odczytu i wysyłki obejmuje całość żądania i korzysta z `write_millis`.
20 poprawnych operacji na konto w oknie 10 sekund; limit jest trwały i wspólny
dla READ/WRITE/SEARCH. Reconnect i restart go nie zerują. Cofnięcie UTC opóźnia
odnowienie budżetu. Błędne pakiety dodatkowo podlegają ogólnemu limitowi transportu.
Duże obciążenie i pomiary wydajności pozostają osobnym etapem.
