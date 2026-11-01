# GG6.1 — jednoznaczny koniec zapisu listy kontaktów

Dev.25 dodaje poprawkę do lokalnego generatora klienta, instalatora oraz
tej samej recepty w przeglądarce. Jest przeznaczona dla rGG. Kod serwera,
atomowość archiwum i jego zasady potwierdzania nie zostały zmienione.

## Przyczyna i zachowanie

Funkcja oryginalnego GG6.1.0.158 wysyła fragmenty do 2047 B. Dodatnia wielokrotność
2047 kończyła się pełnym fragmentem, którego serwer nie może odróżnić od
początku dalszych danych. rGG czeka na krótki lub pusty ostatni fragment;
milczenie klienta ani EOF nie publikują niedokończonej listy.

Nowy klient dopisuje dokładnie jeden pusty USERLIST_PUT_MORE po takim pełnym
zakończeniu. Pusta lista nadal ma jeden pusty PUT, krótkie fragmenty pozostają
niezmienione. Zachowano oryginalny limit 50000 B i zachowanie NULL. Nie zwiększono
limitu ani nie zgaduje się końca na podstawie czasu.

Poprawka zwiększa również licznik oczekiwanych odpowiedzi. Statyczny przegląd
odbiornika w zakresie 0x426542–0x42656a potwierdził, że odpowiedzi typu 0 i 2 zmniejszają
pole obiektu +0x9c, a ukończenie jest zgłaszane po osiągnięciu zera. Ten sam
licznik zwiększa uploader. To analiza konkretnego pliku, nie test całego GUI.

## Zakres i pochodzenie

Oryginał jest przyjmowany tylko po zgodności rozmiaru 790528 B i SHA256
74b78af318c52f86dff8a7ee27a7a0b5ea545167aebb594a8a559ec97f2e3a57.
Źródło instalatora i jego hash zachowano w config/client.example.toml oraz
receipt.json. Żaden oryginalny EXE ani pełny disassembly nie trafił do repo.

Recepta zyskała trzy jawne zakresy (razem 27): 4 B VirtualSize sekcji .text,
9 B skoku i 128 B uprzednio zerowego miejsca. W tym miejscu znajduje się 82 B
własnego kodu x86. Relative CALL/JMP wracają do istniejącego wysyłania pakietu
i epilogu. Układ stosu, rejestry zachowywane przez funkcję i wynik EAX są
zachowane. Pozostałe pola nagłówka PE, rozmiar pliku, instalator i jego pliki
poza gg.exe pozostają niezmienione. Opisuje to sprawdzoną receptę dla jednego
haszowanego klienta, nie ogólny mechanizm patchowania dowolnego programu.

Kod poprawki napisano niezależnie. Fakty binarnego ABI i formatu GG są oddzielone
od kodu programów referencyjnych w THIRD_PARTY.toml. Walidator CPU jest osobnym
programem MIT z własnym LICENSE. Korzysta z zewnętrznego Unicorn 2.1.4, bez
dołączania Unicorna do repo lub runtime rGG i bez importów kodu rGG do emulatora.
Generator/testy rGG pozostają AGPL. Pochodzenie GG32 i innych istniejących
elementów nie uległo zmianie.

## Dowody

- Samodzielny validation/gg6_upload_cpu/check.py: 79 przypadków, w tym wszystkie
  wielokrotności 2047 do limitu, ich sąsiedzi, pusta lista, NULL i 65535 B wejścia.
  Przechwycone pakiety muszą być identyczne z oryginałem plus właściwy pojedynczy
  pusty MORE. Kontrolowane są rejestry/stos, licznik odpowiedzi, EAX i pamięć.
- CPU mapuje tylko sprawdzaną funkcję, miejsce poprawki i ograniczone dane.
  strlen/memcpy/send są zastąpione kontrolowanymi funkcjami. Nie ma loadera
  Windows, wywołań jego API ani sieci. Raport client-upload-cpu-2026-11-01-dev25.json.
- tests/client_upload_wire.py odtwarza osiem dokładnie przechwyconych sekwencji
  przez prawdziwe TCP do rGG. Stara lista pozostaje widoczna przed ostatnim
  fragmentem; po nim przychodzą wszystkie ACK, GET oddaje dokładne bajty,
  a restart zachowuje listę. Raport client-upload-wire-2026-11-01-dev25.json.
- tests/client_package.py z dostarczonym oryginałem: 5 testów parsera/publikacji,
  trzy adresy, identyczny instalator poza podmienionym klientem i 464 odczytane
  zasoby Windows LoadStringW. LoadLibraryEx używa wyłącznie trybu danych/zasobów.
- tests/client_browser.py: 6 pełnych porównań JS/Python i błędnych wejść;
  nowa mapa JSON jest generowana z tej samej recepty, nie utrzymywana ręcznie.
- Rust/HTTP i pakiet: patrz VERIFICATION-2026-11-01-dev25.md.

## Granice kwalifikacji

Oryginalny klient bez tej poprawki nadal nie ma jednoznacznego zakończenia
dla wielokrotności 2047. Nie kwalifikowano innych klientów/serwerów ani całego
GG6.1 w Windows. Instalator nadal korzysta ze standardowego profilu GG i nie
zapewnia izolacji od istniejącej instalacji; do natywnej próby potrzebne jest
oddzielne środowisko lub profil. Nie uruchomiono instalatora ani całego GG,
nie podmieniono zainstalowanych plików i nie zmieniono DNS/hosts/usług.

Lokalny pakiet dla 127.0.0.1 znajduje się w workspace/rgg-client-local-2026-11-01-dev25.
GG.exe: b5e5c51f61467a07a0dce1079c45bfd4596cb3659aeb8a5b37843ad53faaaaeb.
GG-setup.exe: 166f31d95505a090750a7f0da09fe93ed4067e694dda6e6c88a57a3b7aaeb833.
Nie publikowano tych plików. Ostatni kwalifikowany obraz/R0 pozostaje dev.24;
zawiera wcześniejszą receptę klienta. Wynik R0 nie jest przypisywany dev.25.
