# Niezależny klient libgadu 1.7.0

`probe.c` i `build.py` są własnymi, samodzielnymi programami MIT. Klient używa API
zewnętrznej biblioteki libgadu 1.7.0 (LGPL 2.1), bez dołączania kodu rGG.
Sterownik `tests/libgadu_client.py` ma licencję AGPL projektu i komunikuje się
z klientem przez stdin/stdout oraz niezmieniający pakietów lokalny relay TCP.

Biblioteka, jej źródła i binarki nie są dołączone do repo ani obrazu serwera.
Oficjalne archiwum, wskazane przez stronę projektu:
https://github.com/wojtekka/libgadu/releases/download/1.7.0/libgadu-1.7.0.tar.gz

- Rozmiar: 382784 B.
- SHA256: `13b0c4c1471f40022db3fd7d141d5d0b42dd67f0a9ca82db877c39b60e14071a`.
- Plik `COPYING` i nagłówki autorów pozostają w prywatnym katalogu źródeł.

Wersja 1.7.0 faktycznie wysyła LOGIN60; sterownik kontroluje typ 0x15, wersję
0x20 oraz długość pakietu na TCP. Sprawdzane 1.8.2 i 1.11.4 wysyłają już LOGIN70
lub LOGIN80. Nie zmieniamy tych typów w kodzie biblioteki, aby uzyskać test GG6.

## Powtórzenie na Linux/WSL

Potrzebne są Python 3.10+, gcc, make, unshare i ip. Archiwum należy wcześniej
pobrać pod podaną ścieżkę. Builder nie korzysta z sieci i nie uruchamia install.
Weryfikuje hash, ścieżki, typy i rozmiary plików archiwum; po kompilacji porównuje
wszystkie oryginalne pliki ze źródłem. OpenSSL, pthread resolver i bind są wyłączone.
Poniższe katalogi wynikowe muszą być nowe. Polecenia uruchamia się z repo:

```sh
rgg_outer_net=$(readlink /proc/self/ns/net)
unshare -Urn python3 validation/libgadu_170/build.py \
  --archive /path/to/libgadu-1.7.0.tar.gz --output /tmp/rgg-libgadu170-new \
  --outer-network-namespace "$rgg_outer_net"
unshare -Urn sh -c 'ip link set lo up && exec "$@"' rgg-libgadu \
  python3 tests/libgadu_client.py \
  --server /path/to/rgg-server --admin /path/to/rgg-admin \
  --probe /tmp/rgg-libgadu170-new/probe --report /tmp/rgg-libgadu170-new/test.json \
  --outer-network-namespace "$rgg_outer_net"
```

Osobna przestrzeń użytkownika/sieci zawiera tylko loopback. Sam tester ma stały
adres 127.0.0.1, syntetyczne UIN 1–64 i hasła, brak DNS/discovery/TLS/DCC, limit
90 sekund. Nie odczytuje kont prawdziwego użytkownika. Raport zawiera metadane
logowania, lecz nie zapisuje odpowiedzi challenge/hash hasła z pakietu LOGIN60.

Test obejmuje złe hasło, początkowe opisy CP1250, listę subskrypcji, zmianę opisu,
niewidoczność, obustronną rozmowę oraz trzy wiadomości offline po SIGKILL/restart.
Przed zakończeniem synchronizacji prywatności kolejka pozostaje niedostarczona.
Sprawdzane są treść, FIFO, klasy wiadomości, ACK, zamknięcie po SIGTERM i baza.

Osobny `tests/libgadu_directory.py` przyjmuje te same argumenty (z nowym plikiem
raportu). Sprawdza zapis, odczyt, wyszukiwanie, usunięcie i trwałość profilu,
polskie znaki, filtr wieku i dostępności oraz prywatność niewidocznego użytkownika.
Import/eksport kontaktów obejmuje 0, 24, 2046 i 2048 bajtów CP1250, izolację list
i restart. Dokładna wielokrotność 2047 bez terminatora pozostaje niejednoznaczna
w niezmienionej bibliotece; ten test nie oznacza jej kwalifikacji.

`tests/libgadu_media.py` korzysta z tych samych argumentów. API konferencji wysyła
po jednej wiadomości do dwóch odbiorców: jeden odbiera online, drugi po awarii
i restarcie. Sprawdzane są kontekst uczestników, brak dodatkowych kopii i baza.
API obrazków oferuje i przesyła 5120 syntetycznych bajtów w trzech fragmentach;
wynik jest porównywany bajtowo, a CRC niezależnie przez Python zlib. Test obejmuje
obcego odbiorcę, ofertę offline po restarcie, brak nadawcy i późniejszy odbiór od
niewidocznego nadawcy. Nie testuje dekodowania/renderowania pliku graficznego.

`tests/libgadu_paging.py` publikuje przez API 41 profili (UIN3–43) i odczytuje
je jako UIN2, korzystając z `gg_pubdir50_next` i `GG_PUBDIR50_START`. Sprawdza
strony20/20/1, kursory23/43/0, komplet numerów bez duplikatów i pustych wierszy,
polskie pola oraz powtórzenie po restarcie. Relay nadal dopuszcza najwyżej
cztery pary gniazd jednocześnie; ten test ma jawny limit48 połączeń łącznie.
Raport zapisuje również rzeczywiste image_size z LOGIN60, system i kodowanie.

Nie jest to wykonanie oryginalnego GUI GG6 ani test automatycznego reconnect.
Nie obejmuje jeszcze całej macierzy błędnych obrazków czy czasu
powrotu w API libgadu. Testy są uruchamiane jawnie, poza zwykłą CI; regresja
opisów jest również w standardowym `gg6_presence_contacts.py` i testach Rust.
