# Lokalny pakiet klienta — dev.17

`tools/build_client.py` (Python 3.11+) przygotowuje osobny `gg.exe`, instalator
`gg-setup.exe`, manifest pobierania, instrukcję i potwierdzenie hashy. Nie pobiera
ani nie uruchamia oprogramowania. Katalog wyjściowy musi być nowy. Oryginalny
plik, zainstalowany GG, profile, DNS, hosts i usługi pozostają nietknięte.

## Użycie

Skopiuj `config/client.example.toml` i ustaw `hostname`, `source_url` oraz
informację o licencji. Adres klienta ma 1–16 znaków ASCII; bez schematu i portu.
Domyślnie jest to `127.0.0.1`. Narzędzie przyjmuje wyłącznie wskazany poniżej
oryginalny instalator GG 6.1.0.158, dostarczony osobno przez operatora.

```sh
python tools/build_client.py --installer /path/to/original/gg61.exe --config config/client.example.toml --output /path/to/new-package
```

Ustaw `downloads_manifest` serwera na wygenerowany `manifest.toml`, by podawać
pliki przez istniejący katalog WWW. Manifest jest publikowany na końcu, po
zapisaniu i sprawdzeniu pozostałych plików. Nieukończony katalog nie jest używany
ponownie; kolejna próba wymaga innego katalogu.

Adres klienta jest niezależny od konfiguracji nasłuchu serwera. Stary GG szuka
huba przez HTTP na porcie 80; domyślny rGG ma port 8088. Nie wykonywano przekierowań
portów. Próba prawdziwego klienta wymaga osobno przygotowanego lokalnego huba i
izolowanego środowiska. Oryginalny instalator korzysta ze standardowych ustawień
i profilu GG; nie zapewnia osobnego profilu obok istniejącej instalacji.

## Ścisły zakres zmian

- Cały źródłowy instalator musi mieć SHA-256
  `bcc8157aa6bface009d8018c308bf3cef8725546b4f826bdbaf6bbeaa953b06f`.
  Wydobyty klient: 790528 B, SHA-256
  `74b78af318c52f86dff8a7ee27a7a0b5ea545167aebb594a8a559ec97f2e3a57`.
- Wskazane nazwy serwerów są zastępowane w ASCII, UTF-16 i tablicach RT_STRING.
  Tablice zachowują 16 elementów i poprawne długości. Końcówki URL/HTML oraz
  następne teksty nie są ucinane po skróceniu nazwy hosta.
- Trzy wywołania starego aktualizatora zwracają zero, zachowując kontrakt cdecl.
  Metadane importów i dostarczana biblioteka pozostają na miejscu.
- Domyślne nazwy dźwięków to `wiadomosc` i `dostepny`, zgodne z plikami WAV
  instalatora. Nie zmienia się ustawień już istniejącego profilu użytkownika.
- Przebudowywany jest solid-LZMA konkretnego instalatora NSIS, z nowym rozmiarem
  i CRC. Stub, skrypt, oryginalna licencja, DLL, dźwięki i pozostałe bajty
  rozpakowanego ładunku pozostają identyczne.

Odczyty mają granice rozmiaru, zakresów i głębokości struktur. Inny hash,
nakładające się poprawki, niezgodne stare bajty albo plik już zmodyfikowany są
odrzucane. To przepis dla jednej wersji, nie uniwersalny patcher PE/NSIS.
`receipt.json` dokumentuje wersję narzędzia, zakresy zmian oraz hashe wejścia
i wyjść; jawnie wskazuje `executed=false` i `native_client_qualified=false`.

## Pochodzenie i testy

Fakty zgodności (wersja, offsety, ABI, nazwy dźwięków) sprawdzono w przypiętym
forku gg-retro `ed9640cc247764754b5e47891053eb07a53f76d5`. Implementacja parserów,
patchera i generatora jest własna; nie skopiowano jego kodu Python/JavaScript.
Format nagłówka sprawdzono w [źródle NSIS](https://github.com/kichik/nsis/blob/f0bfe3e0062c57dc1ef90f1d235c7c0bd5a43be4/Source/exehead/fileform.h)
i [opisie kompresji solid](https://nsis.sourceforge.io/Reference/SetCompressor).
Wpisy referencyjne są w `THIRD_PARTY.toml`.

Kod narzędzi jest na AGPL; oryginalny klient i instalator mają własne warunki.
Pliki EXE nie są dodawane do Git. Źródłowy URL jest metadanymi operatora,
nie poleceniem pobrania ani dowodem prawa do redystrybucji.

```sh
python tests/client_package.py
python tests/client_package.py --installer /path/to/original/gg61.exe
python tests/client_downloads.py --server target/debug/rgg-server --admin target/debug/rgg-admin --manifest /path/to/new-package/manifest.toml
```

Pierwszy test jest syntetyczny i należy do CI. Drugi jest opcjonalny: trzy hosty,
granice poprawek, identyczny pozostały ładunek i niezmienione źródło. Na Windows
używa [LoadLibraryExW](https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-loadlibraryexw)
z `LOAD_LIBRARY_AS_DATAFILE_EXCLUSIVE | LOAD_LIBRARY_AS_IMAGE_RESOURCE` oraz
[LoadStringW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-loadstringw).
Nie ładuje importów ani nie wywołuje kodu GG. Trzeci test pobiera pliki wyłącznie
przez loopback, sprawdza SHA-256, HEAD, instrukcje i równoczesne logowanie GG.

Pozostają: uruchomienie instalatora i GG6 w izolowanym środowisku, sprawdzenie
dźwięków/aktualizatora w runtime oraz kwalifikacja połączenia. Patcher w przeglądarce
nie jest zaimplementowany; przygotowanie pakietu odbywa się przez CLI.
