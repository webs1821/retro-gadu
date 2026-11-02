# Dev.28 — adres GG za pośrednikiem

Dodano opcjonalny `gg_advertise`, oddzielny od `gg_bind`. Rzeczywiste nasłuchy
pozostają ograniczone do loopback. Schema13 i190 wersji zewnętrznych bibliotek
bez zmian. [Opis kontraktu](ADVERTISED-ENDPOINT.md).

Przeszły113 testów Rust, fmt, Clippy wszystkich targetów/features i normalny
build. Test discovery_proxy przeszedł15 odmów błędnej konfiguracji przed
utworzeniem bazy oraz4 połączenia przez lokalny relay (534 przekazane bajty):
HTTP/1.0, obce nagłówki, logowanie, online, offline i restart ze zmianą portu
zaplecza. Dotychczasowy http_discovery także przeszedł bez nowej opcji.

Po tych wynikach wykonano dodatkową regresję:

| Grupa | Czas |
|---|---:|
| Lifecycle/lock/migracje/odmowa publicznego bindu | 0,251s |
| Sesje GG6, offline, prywatność i limity | 5,567s |
| 42 komendy w niewłaściwych stanach i deadline | 4,007s |
| Rejestracja WWW | 6,405s |
| Zmiana hasła WWW | 2,398s |

Raport advertised-endpoint-2026-11-02-dev28.json rozróżnia wcześniej wykonane
kontrole (wynik obserwowany w narzędziu) oraz pięć regresji z zapisanym stdout
i stderr. Testów już zakończonych nie powtarzano wyłącznie do zebrania logu.
W raporcie są hashe źródeł, sterowników i binarek; sprawdzono je po zakończeniu.

- Serwer debug:22efb6a346685965175c7a2d414115630a2c4ef5bb7ee866f6fc0be80ff5e948.
- Admin debug:631c1caad9d265f940fd947491d967731834fee70bcbba856d9a4c0b8bc4d80f.
- Zachowany baseline dev.27:/tmp/rgg-baseline-dev27.
- 11 wpisów pochodzenia,10 modułów Rust,13 migracji; pierwotny plan niezmieniony.

Pośrednik i serwery testowe zakończyły pracę. Nie zmieniano DNS/hosts, klienta,
routera ani działającego gg-retro. Brak push/deploy, nowego obrazu i kwalifikacji
natywnego GG6. Ostatni kwalifikowany obraz nadal dev.26; publiczny profil WWW/TLS
jest osobnym, otwartym zadaniem.
