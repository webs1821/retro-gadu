# Profil strony WWW za lokalnym proxy HTTPS — dev.30

Opcjonalny profil oddziela adres przeglądarki od lokalnego nasłuchu rGG.
Serwer nadal akceptuje tylko bindy loopback. Nie uruchamia TLS, proxy ani DNS,
nie pobiera certyfikatów i nie zmienia sieci. W tej wersji obsługujemy jeden
zaufany proxy w tej samej przestrzeni sieciowej co backend. Zwykły profil
lokalnego HTTP pozostaje domyślny.

```toml
http_bind = "127.0.0.1:8088"
gg_bind = "127.0.0.1:8074"
web_registration = true
web_password_change = true
web_public_stats = true

[web_proxy]
origin = "https://rgg.example.test:8443"
trusted_proxy = "127.0.0.2"
```

To fragment konfiguracji, wymagający także data_dir i przygotowanego keyringu
jak zwykłe formularze. Przynajmniej jedna funkcja WWW musi być włączona.
`web_proxy` nie włącza rejestracji, zmiany hasła ani statystyk automatycznie.
Nie zmienia też adresu ogłaszanego klientom GG przez `gg_advertise`.

## Origin i tożsamość

Origin jest jawny i nie pochodzi z Host ani nagłówków proxy. Obsługiwany zapis:
`https://host[:port]`, bez ścieżki, końcowego ukośnika, query, fragmentu lub
danych logowania. Host DNS jest małymi literami ASCII, maksymalnie 253 znaki,
etykiety 1–63 znaków z liter/cyfr/myślników, bez myślnika na końcu/początku.
Końcowa etykieta DNS zawiera tylko litery; Unicode i IDN z punycode w końcowej
etykiecie nie są obecnie obsługiwane. Pozostałe etykiety mogą używać punycode.
Alternatywą jest kanoniczny IPv4 albo IPv6 w nawiasach, bez zone ID i bez IPv4
mapped IPv6. Odrzucane są skrócone/ósemkowe/szesnastkowe zapisy IPv4 i aliasy
IPv6. Domyślny port 443 należy pominąć, pozostałe muszą być 1–65535 bez zer
wiodących. Konfiguracja nie wykonuje zapytań DNS. To celowo ograniczony format
konfiguracji, nie ogólny parser wszystkich poprawnych URL.

Każda włączona trasa WWW wymaga jednego Host zgodnego z authority z origin.
POST wymaga jednego Origin zgodnego z konfiguracją. Jeżeli GET zawiera Origin,
też musi się zgadzać. Sec-Fetch-Site może być nieobecny albo pojedynczy
same-origin/none. Duplikaty tych pól są odrzucane. Reguła opcjonalnego Origin
i jednoznacznego Sec-Fetch-Site obowiązuje także zwykłe lokalne HTTP.

W profilu proxy bezpośredni peer TCP musi być równy `trusted_proxy`. Ponadto
wymagane są pojedyncze `X-Forwarded-Proto: https` i `X-Real-IP` zawierający
wyłącznie adres IP. Nie są akceptowane listy, porty, nawiasy, zone ID ani nazwy.
IPv4 mapped IPv6 jest tutaj normalizowany do IPv4, aby nie rozdzielać limitów
i tokenów tego samego klienta. Forwarded, X-Forwarded-For i X-Forwarded-Host
nie uczestniczą w wyznaczaniu tożsamości ani adresu strony.

Zweryfikowane IP jest zapisane w rozszerzeniu żądania `WebsiteClient` i używane
przez wspólny limit oraz oba formularze. Fizyczny ConnectInfo pozostaje adresem
proxy. W zwykłym lokalnym HTTP wszystkie nagłówki przekazania są ignorowane,
a tożsamość pochodzi bezpośrednio z TCP. Nie ma automatycznego fallbacku
z profilu proxy do lokalnej domeny lub IP, jeżeli brakuje wymaganych nagłówków.

W obu formularzach ciasteczka zachowują HttpOnly, SameSite=Strict, ścisły Path,
brak Domain i ważność 600 s. Profil HTTPS dodaje Secure także przy kasowaniu
ciasteczka. Tokeny nadal są jednorazowe, związane z celem formularza oraz IP.
Zmiana hasła zachowuje pełne obecne hasło, trwały limit konta i barierę sesji.

## Umowa z operatorem proxy

Proxy musi sam zakończyć zweryfikowany TLS i **nadpisywać**, a nie dopisywać,
nagłówki tożsamości od klienta. Przykładowe dyrektywy nginx w już przygotowanym
serwerze HTTPS (szkic, nie wdrożona ani sprawdzona instancja nginx):

```nginx
proxy_bind 127.0.0.2;
proxy_set_header Host $http_host;
proxy_set_header X-Real-IP $remote_addr;
proxy_set_header X-Forwarded-Proto $scheme;
proxy_set_header Forwarded "";
proxy_set_header X-Forwarded-For "";
proxy_set_header X-Forwarded-Host "";
proxy_pass http://127.0.0.1:8088;
```

Host zachowuje wejściową wartość, żeby backend mógł odrzucić obcą domenę.
Certyfikat, ograniczenia TLS, body/gniazd, timeouty i ewentualny HSTS pozostają
odpowiedzialnością konfiguracji proxy. Prywatnego listenera metryk nie wolno
łączyć z publiczną stroną. Trasy health i legacy discovery nie należą do
warstwy WWW i zachowują własne dotychczasowe reguły; przyszła konfiguracja
zewnętrznego proxy musi jawnie wybrać trasy, które udostępnia.

Zaufanie do IP loopback jest granicą jednego hosta/przestrzeni sieciowej,
nie uwierzytelnieniem kryptograficznym proxy. Inny proces mający możliwość
połączenia ze wskazanego adresu może podszyć się pod proxy. Nie wspieramy tu
zdalnych proxy, list CIDR, automatycznych łańcuchów XFF ani PROXY protocol.
Limity gniazd backendu są liczone według prawdziwego peer TCP (czyli proxy),
a limity WWW i CSRF według zweryfikowanego IP klienta. Limity globalne pozostają.
Nie jest to kompletna kwalifikacja publicznej ekspozycji ani DDoS.

## Weryfikacja i pochodzenie

Test `tests/http_proxy.py` wymaga CLI OpenSSL do jednorazowego certyfikatu.
Tworzy własny, ograniczony proxy TLS na loopback z jednym dołączanym wątkiem.
Klient łączy się na lokalny adres bez DNS, ale weryfikuje nazwę rgg.test oraz
certyfikat. Systemowy magazyn zaufania nie jest zmieniany. Klucz, certyfikat
i baza są w usuwanym katalogu tymczasowym. Nie uruchamia się nginx.

Test obejmuje odmowę obcej nazwy i niezaufanego certyfikatu, Secure przy
wydawaniu/kasowaniu cookies, powiązanie IP/replay obu formularzy, rejestrację,
zmianę hasła i stare/nowe logowanie GG, statystyki, pobieranie, dokładne zasoby
patchera, odseparowanie metryk, health i discovery. Sprawdza też obcy peer,
złe/duplikowane/brakujące nagłówki, normalizację IP, oddzielne budżety klientów,
brak obejścia przez nagłówki i 13 błędnych konfiguracji przed utworzeniem bazy.

Własna implementacja i testy, bez portowania kodu innych projektów. Fakty
interfejsów zweryfikowano w [RFC6454](https://www.rfc-editor.org/rfc/rfc6454),
[RFC6265](https://www.rfc-editor.org/rfc/rfc6265) i dokumentacji
[nginx proxy_set_header](https://nginx.org/en/docs/http/ngx_http_proxy_module.html#proxy_set_header).
Pochodzenie: `https-proxy-reference` w THIRD_PARTY.toml.
