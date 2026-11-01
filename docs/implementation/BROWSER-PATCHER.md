# Przygotowanie klienta w przeglądarce — dev.20

Opcjonalny formularz `/patcher` przygotowuje nową kopię oryginalnego `gg.exe`
z GG **6.1.0.158**. Wszystkie operacje na pliku odbywają się w przeglądarce.
Nie istnieje endpoint przyjmujący pliki. Instalator nadal tworzy narzędzie
CLI opisane w [CLIENT-PACKAGES.md](CLIENT-PACKAGES.md).

## Włączenie lokalne

Dodaj osobną tabelę na końcu konfiguracji serwera:

```toml
[client_patcher]
default_host = "127.0.0.1"
```

Brak tabeli wyłącza stronę i trzy zasoby narzędzia. `default_host` jest
walidowany przed otwarciem bazy: 1–16 znaków ASCII, litery/cyfry i pojedyncze
separatory `.` lub `-`; bez separatora na początku/końcu. Nie jest to test DNS.
Wymagane jest przygotowanie połączeń klienta pod wybranym adresem — narzędzie
nie zmienia DNS, hosts, portów, ustawień aktualnego GG ani konfiguracji serwera.
Nasłuch rGG pozostaje wyłącznie loopback.

## Granice i kontrakt

- Przyjmowana jest dokładnie jedna wersja pliku: 790528 B, SHA-256
  `74b78af318c52f86dff8a7ee27a7a0b5ea545167aebb594a8a559ec97f2e3a57`.
  Inny lub już zmieniony plik jest odrzucany. Rozmiar sprawdzany przed odczytem.
- Własny moduł ES bez zależności npm sprawdza SHA-256 przez Web Crypto,
  kopiuje wejście i mapę zmian przed operacjami asynchronicznymi oraz odmawia
  nakładających się/niepoprawnych zakresów i niespełnionych warunków patcha.
- 24 zakresy: 3 tablice RT_STRING, 17 łańcuchów C i 4 stałe poprawki
  aktualizatora/dźwięków. Tablice mają przebudowane prefiksy długości UTF-16;
  zachowane są kolejne teksty, ogony URL i bajty poza zakresami zmian.
  Łańcuchy jednobajtowe nie są przekodowywane przez UTF-8.
- Mapa JSON zawiera offsety, długości, domeny i minimalne stałe sekwencje ABI.
  Nie zawiera klienta ani kopii jego tablic tekstowych. Własny generator
  `tools/client_plan.py` wymaga przypiętego oryginalnego instalatora i czyta
  najwyżej 2 MiB + 1 B. Tryb `--check` porównuje mapę semantycznie.
- Pobranie następuje z lokalnego Blob URL jako nowy `gg.exe`. Zmiana pliku lub
  adresu unieważnia poprzedni link; również opuszczenie strony zwalnia URL.
  Nie ma automatycznego uruchomienia, podmiany ani przesyłania pliku.
- Strona wymaga nowoczesnej przeglądarki z ES modules, File API i Web Crypto
  w bezpiecznym kontekście (testowany lokalny HTTP loopback). Stary IE nie
  jest odbiorcą tego narzędzia. Po stronie serwera pozostają te same limity
  gniazd, czasu i wspólny budżet WWW.
- Dokładny Host, opcjonalny Origin i Sec-Fetch-Site chronią lokalny endpoint.
  Query, ciało GET/HEAD i Transfer-Encoding są odrzucane; POST nie przyjmuje
  pliku. Formularz nie ma nazw pól i nie ma trybu zapasowego uploadu.
  CSP ze skryptami wyłącznie `self` dotyczy tylko tej strony; rejestracja,
  statystyki i pobieranie zachowują politykę bez JavaScript.

## Sprawdzenie i pochodzenie

`tests/client_browser.py` uruchamia test ES w Node (sprawdzono 24.11.1).
Domyślnie używa wyłącznie syntetycznego PE; opcjonalny `--installer` dodaje
trzy pełne porównania bajtowe z niezależną implementacją Python na oryginalnym
GG. Żaden test nie wykonuje klienta. CI używa zainstalowanego Node 22+;
obraz testowy Pythona nie zawiera Node, więc ten zestaw jest wykonywany osobno.

`tests/http_patcher.py` sprawdza prawdziwe lokalne HTTP, niezależny opt-in,
osadzone zasoby, nagłówki, CSP, brak uploadu, błędy konfiguracji, limity i
integrację nawigacji. Dodano go również do sterownika przyszłego obrazu;
istniejący obraz dev.19 nie zawiera tej funkcji.

Kod JavaScript jest własną implementacją recepty rGG. Fakty konkretnego
GG6 pochodzą z jawnie przypiętej referencji gg-retro; nie skopiowano jego
patchera JS. Zasady Web API sprawdzono w [File API W3C](https://www.w3.org/TR/FileAPI/)
i [Web Cryptography](https://www.w3.org/TR/webcrypto/). Pełne przypisanie
w [THIRD_PARTY.toml](../../THIRD_PARTY.toml).

Wyniki: [raport dev.20](VERIFICATION-2026-11-01-dev20.md). Sprawne przygotowanie
pliku nie kwalifikuje uruchomienia oryginalnego GG ani zgodności serwera z nim.
