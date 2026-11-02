# Oddzielny adres ogłaszany klientom GG6 — dev.28

Opcjonalne `gg_advertise` określa IPv4 i port podawane przez legacy HTTP discovery.
`gg_bind` nadal określa rzeczywisty nasłuch. Umożliwia to oddzielenie portu rGG od
adresu pośrednika TCP lub przekierowania portów. Wartość pochodzi wyłącznie
z konfiguracji operatora; nie zależy od Host, X-Forwarded-* ani parametrów GET.

Przykładowy fragment konfiguracji lokalnej:

```toml
gg_bind = "127.0.0.1:8074"
legacy_discovery = true
gg_advertise = "127.0.0.2:18074"
```

Gdy rGG jest Ready, appmsg3.asp i appmsg4.asp zwracają wtedy
`0 0 127.0.0.2:18074 127.0.0.2` z CRLF. Operator musi osobno zapewnić pośrednik
prowadzący do właściwego nasłuchu. rGG nie uruchamia proxy, nie sprawdza jego
osiągalności i nie rozwiązuje DNS. Przy braku Ready pozostaje `notoperating`.

Bez `gg_advertise` zachowano dotychczasowy adres rzeczywistego listenera,
także przy porcie0. Nowa opcja wymaga `legacy_discovery=true` i lokalnego
nasłuchu IPv4 GG. Akceptuje wyłącznie numeryczny IPv4 z niezerowym portem;
odrzuca0/8 oraz224/4 i240/4, w tym adres nieokreślony i ograniczony broadcast.
Walidacja nie gwarantuje routowalności ani osiągalności skonfigurowanego adresu.

Typ `rgg_http::AdvertisedEndpoint` waliduje wartość przy deserializacji,
również gdy jest budowany bezpośrednio przez API. Serwer sprawdza konfigurację
przed otwarciem katalogu danych. Błędy TOML nadal nie wypisują treści wejścia.

Wszystkie rzeczywiste nasłuchy HTTP/GG/metrics nadal muszą być loopback.
Opcja nie zmienia tej polityki, reguł WWW, starych operacji kont HTTP403,
konfiguracji DNS/hosts ani plików klienta. Publiczny profil WWW/TLS/zaufanego
proxy i wdrożenie pozostają osobnymi pracami.

## Sprawdzenie

Test `discovery_proxy.py` uruchamia ograniczony pośrednik tylko na127.0.0.2
i przekazuje ruch do127.0.0.1. Ma najwyżej8 par połączeń, odczyty do64KiB,
timeout zapisu1s i kontrolowane zakończenie wątku oraz gniazd.
To fixture testowa, nie komponent produkcyjny.

Przeszły HTTP/1.0, oba endpointy discovery, obce nagłówki/query, logowanie,
wiadomość online, kolejka offline, restart z nowym portem zaplecza i odczyt
tej kolejki przez ten sam ogłaszany adres. Sprawdzono15 błędnych konfiguracji,
każdą przed utworzeniem bazy. W zwykłym trybie bez opcji przeszedł dotychczasowy
test HTTP discovery. Test używa syntetycznego klienta; natywny GG6 niekwalifikowany.

Brak migracji bazy (schema13) i zmian zewnętrznych bibliotek. Zmieniono sygnaturę
wewnętrznie używanego `HttpState::with_legacy_discovery`: drugi argument jest
opcjonalnym walidowanym adresem. Normalne binarki dev.27 zachowano jako baseline.
Ostatni kwalifikowany obraz pozostaje dev.26; ta funkcja nie była na nim testowana.
