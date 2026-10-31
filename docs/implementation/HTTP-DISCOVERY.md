# Lokalne HTTP discovery — dev.6

Po włączeniu `legacy_discovery = true` w głównej sekcji konfiguracji endpointy
`/appsvc/appmsg3.asp` i `/appsvc/appmsg4.asp` podają rzeczywisty adres aktywnego
nasłuchu GG. Przykład odpowiedzi: `0 0 127.0.0.1:8074 127.0.0.1` z CRLF.
Port pochodzi z otwartego listenera, więc działa także konfiguracja portu 0.
Nie jest wyliczany z Host, X-Forwarded-* ani parametrów zapytania.

Opcja domyślnie wyłączona. Wymaga IPv4 loopback dla gg_bind. Gdy discovery jest
wyłączone lub serwer nie jest Ready, odpowiedź zawiera `notoperating` zamiast
adresu. HTTP/1.0 i zamknięcie po odpowiedzi są obsługiwane. Nagłówki ustawiają
no-store, Connection: close, jawny typ tekstowy i nosniff. Nie ogłaszamy obsługi
nowego protokołu przez appmsg_ver8.asp.

Format opracowano niezależnie na podstawie [dokumentacji libgadu](https://libgadu.github.io/protocol/),
wpis `libgadu-wire-reference` w THIRD_PARTY.toml. Nie kopiowano implementacji.
HTTP nie nadpisuje systemowego DNS/hosts, nie patchuje klienta i nie wysyła
żadnych żądań do oficjalnej infrastruktury GG. Natywny GG nadal wymaga osobnego
środowiska kwalifikacyjnego; testowany klient protokołu jest syntetyczny.

Stare operacje konta pod `/appsvc/fmregister3.asp`, `/fmregister.php` i
`/fmsendpwd.php` zawsze odmawiają HTTP 403. Handler nie odczytuje hasła ani
zaproszenia z formularza i nie wywołuje portu kont. Nie wolno przez te ścieżki
ominąć rejestracji z zaproszeniem ani zmienić hasła. W dev.7 rejestrację z zaproszeniem realizuje osobny [formularz WWW](WEB-REGISTRATION.md).
Bezpieczne odzyskiwanie konta pozostaje do implementacji.

Publiczny nasłuch, domena produkcyjna, proxy i TLS pozostają poza lokalnym
profilem uruchomienia. Opcja discovery nie znosi blokady adresów innych niż loopback.
