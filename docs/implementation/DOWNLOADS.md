# Lokalny katalog pobierania — dev.11

Opcjonalny `downloads_manifest` w konfiguracji wskazuje manifest TOML względnie
do katalogu konfiguracji. Brak opcji oznacza wyłączone endpointy. Katalog może
działać niezależnie od liczników i rejestracji. Strona główna pokazuje odnośnik,
jeśli włączono jednocześnie liczniki i pobieranie. Profil nadal nasłuchuje tylko
na loopback; ten przyrost nie dodaje publicznego wdrożenia.

Endpointy: `/downloads` (opis/instrukcje), `/downloads/{id}` (plik),
`/api/downloads` (publiczny manifest bez lokalnych ścieżek). HTTP GET i HEAD;
żądanie Range zwraca 416, wznawianie pobrań nie jest jeszcze obsługiwane.

## Przykład manifestu

To szablon, nie gotowy pakiet. Należy podać rzeczywisty rozmiar, hash, pochodzenie
i warunki używania; placeholder nie przejdzie kontroli. Binarki są dostarczane
oddzielnie przez administratora i nie stają się częścią kodu na licencji AGPL.
Serwer nie pobiera plików z `source_url` i nie uruchamia ich.

```toml
schema = 1

[[artifacts]]
id = "gg6-client"
kind = "client" # albo "installer"
title = "Klient GG6 — plik do podmiany"
file = "gg.exe" # wyłącznie nazwa pliku obok manifestu
size = 123456
sha256 = "TU_WSTAW_64_MALE_ZNAKI_HEKSADECYMALNE"
source_url = "https://example.invalid/udokumentowane-zrodlo"
license = "Rzeczywiste warunki używania i dystrybucji tego pliku."
instructions = "Zamknij GG. Zrób kopię gg.exe w folderze instalacji, następnie zastąp ten plik. To nie jest instalator."
```

Hashe są kontrolą spójności z decyzją administratora, a nie podpisem autora,
oceną bezpieczeństwa programu lub automatyczną weryfikacją praw do dystrybucji.
Dev.17 dodaje [generator pakietu klienta](CLIENT-PACKAGES.md), który przyjmuje
osobno dostarczony oryginalny instalator o przypiętym hashu i tworzy taki manifest.
Nie skopiowano plików z działającej instancji gg-retro ani nie uruchamiano GG.

## Ładowanie i granice

Manifest jest czytany przed inicjalizacją bazy i uruchomieniem listenerów. Błąd
powoduje odmowę startu ze stałym komunikatem, bez wypisywania zawartości manifestu.
Nieznane pola, duplikaty ID/nazw, ścieżki zamiast nazw plików, symlinki i pliki
inne niż zwykłe są odrzucane. Liczba plików 1–4, manifest do 16 KiB, każdy plik
1 B–32 MiB, suma zadeklarowanych rozmiarów do 64 MiB. Teksty mają osobne limity,
źródło musi być adresem HTTPS, a dane HTML są escapowane.

Każdy plik jest wczytany w ograniczonym odczycie i musi zgadzać się co do długości
oraz SHA-256. Serwer przechowuje zweryfikowaną kopię w RAM: podmiana pliku na dysku
po starcie nie zmienia pobieranej zawartości. Zmiana manifestu/pliku wymaga restartu
i ponownej kontroli. Katalog konfiguracyjny jest zaufany i powinien być zapisywalny
wyłącznie przez administratora; nie stanowi przestrzeni uploadu użytkowników.

To limit zawartości, nie obietnica identycznego RSS: występują narzuty alokatora,
czasowe bufory odczytu i bufory HTTP/systemu. Kopie dla równoległych odbiorców
współdzielą `Bytes`; nie powielają całego pliku. Maksymalnie 4 strumienie plików,
fragmenty po 64 KiB, permit pozostaje z ciałem odpowiedzi do końca/anulowania.
Zamknięcie połączenia oddaje slot. Wolny odbiorca może zajmować slot do rozłączenia;
osobny timeout bezczynnego zapisu HTTP i pomiary obciążenia są bramką publicznego
profilu, którego ta wersja nie deklaruje. Logowanie GG działa przy zajętych slotach.

Każdy endpoint stosuje istniejącą kontrolę Host, Fetch-Site, Ready i wspólną kwotę
źródłowego IP; X-Forwarded-For nie daje dodatkowego budżetu. Odpowiedzi mają
no-store, nosniff, CSP, DENY i no-referrer. Pliki otrzymują octet-stream oraz
Content-Disposition: attachment z kontrolowaną nazwą. ID z URL wybiera tylko
wpis manifestu, nigdy ścieżkę na dysku. Query string nie zmienia wybranego pliku.

## Weryfikacja

`tests/http_downloads.py` używa wyłącznie sztucznych bajtów, nie prawdziwych EXE.
Sprawdza SHA, długość, HTML, nagłówki, HEAD/Range, niezmienność po podmianie, brak
lokalnych ścieżek w JSON, nieprawidłowe manifesty/symlinki, samodzielne włączenie,
budżet, cztery wolne strumienie, odzyskanie slotu i logowanie GG podczas pobierania.
Testy strony/rejestracji/statystyk oraz lifecycle są regresją współdzielonej ochrony.

`tools/preview_registration.py --downloads` tworzy tymczasową bazę i dwa sztuczne
pliki do podglądu UI; Enter kończy proces i usuwa próbki. Nie instaluje oprogramowania.
Sprawdzono widok desktop w przeglądarce; nie deklarowano testu mobilnego/natywnego GG.
