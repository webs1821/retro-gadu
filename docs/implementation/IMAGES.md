# Obrazki GG6 — kontrakt dev.10

Własna implementacja na podstawie [opisu protokołu libgadu](https://libgadu.github.io/protocol/)
i przypiętych nagłówków/handlerów libgadu; wpisy `libgadu-wire-reference` oraz
`libgadu-legacy-reference` w THIRD_PARTY.toml. Nie kopiowano implementacji C.
Natywny GG6 pozostaje do kwalifikacji. Testy używają syntetycznych klientów TCP.

## Podział odpowiedzialności

- `rgg-application::images`: identyfikatory, walidacja, port ofert i transferów.
- `rgg-protocol::gg6_images` oraz richtext w `gg6_messages`: czyste pakiety.
- `rgg-store-sqlite::images`: trwałe, imienne uprawnienia i kontrola obu sesji.
- `rgg-transfers`: ograniczone składanie i weryfikacja obrazu w RAM, bez I/O.
- `SessionHub`: bramka publikacji wspólna z blokadami i wymianą sesji.
- `rgg-transport`: jeden writer na połączenie, deadline każdego zapisu.

Aplikacja nie zależy od konkretnego `rgg-transfers`; serwer składa adaptery.

## Odwołanie w wiadomości i uprawnienie

Wpis richtext z bitem 0x80 zawiera po ewentualnym RGB nagłówek 09 01, rozmiar
i CRC32 LE. Treść obrazka bez tekstu używa NBSP (0xA0 CP1250). Limit to 8 różnych
obrazków w wiadomości, 256 wpisów richtext i dotychczasowe 4096 B pakietu SEND_MSG.
Serde zachowuje kompatybilność dawnych stylów bez pola `image`.

Jedna transakcja zapisuje tekst, style, kontekst konferencji, kolejkę i wszystkie
oferty. Dopiero commit pozwala wysłać ACK 3. Flaga NO_ACK tego nie zmienia.
Oferta wskazuje faktycznego nadawcę i odbiorcę tej kopii; pozostali uczestnicy
konferencji nie otrzymują uprawnienia. Sam rozmiar/CRC nie uprawnia do pobrania.

Schemat 8 dodaje `image_offers` i deklarację klienta w `account_sessions`.
Nie ma FK od oferty do `messages`: usunięcie tekstu po zapisie TCP i ograniczanie
historii nie kasuje dostępu do wcześniej doręczonego odwołania. Oferta ma TTL
wiadomości podczas oczekiwania, następnie 7 dni od `TransportWrittenUnconfirmed`.
Brak odbiorczego ACK w GG6 nadal uniemożliwia deklarację potwierdzonego odbioru.
Wygaśnięcie, odrzucenie, blokada w dowolną stronę lub usunięcie konta usuwa grant;
odblokowanie go nie odtwarza. Granice: 80000 rekordów globalnie i 8000 dla konta
łącznie jako nadawcy/odbiorcy. Limity obejmują także już doręczone wiadomości.

## Pobieranie i prywatność

Pierwszy odbiór wymaga połączonego nadawcy. Każde żądanie i każdy zapis fragmentu
sprawdza obie sesje Ready, epokę, generację, wersję poświadczenia, aktywność kont,
aktualną ofertę, blokady oraz limit odbiorcy. Zmiana sesji przerywa stary transfer;
nowa sesja może ponowić żądanie zachowanej oferty. Brak nadawcy/oferty daje tę samą
pustą odpowiedź, bez ujawniania przyczyny odmowy.

Tryb niewidoczny i „tylko znajomi” nadal ukrywają obecność. Nie odbierają osobno
udzielonego uprawnienia do obrazka; połączeni użytkownicy mogą wymieniać wcześniej
udostępniony obraz tak jak wiadomość. Ukończony transfer może więc potwierdzić
rozmówcy dostępność nadawcy. Blokada bezwzględnie odwołuje uprawnienie.

W polu `image_size` statusu publikowany jest limit z LOGIN60 tylko przy widocznej
obecności Available/Away. Offline/niewidoczny/zablokowany ujawnia 0. IP, port DCC
i fingerprint klienta pozostają zerowe. Nowa sesja nie dziedziczy limitu starej.
Limit 0 oznacza brak zgody na odbieranie obrazków; żądanie o rozmiarze 0 nie jest
obsługiwane jako historyczny keepalive. Własny obrazek do własnego UIN jest odrzucany.

## Ulotne sterowanie i ograniczenia

Pusta wiadomość klasy MSG (4; opcjonalnie NO_ACK 32) niesie osobny typ komendy:
04 — żądanie; 05 — pierwszy fragment z nazwą NUL; 06 — dalsze dane. Nie wolno
mieszać sterowania z tekstem, konferencją ani stylami. Odpowiedź zawierająca tylko
9 B flagi/rozmiaru/CRC oznacza brak obrazka — bez pola nazwy i bez dodatkowego NUL.

ACK sterowania: 2 oznacza przyjęcie do ulotnego przetwarzania, 4 przekroczony limit
lub istniejący transfer, 6 odmowę/błąd. Brak trwałej kolejki sterowania, ACK 3 ani
gwarancji dostarczenia wynikającej z ACK 2. NO_ACK nie wyłącza odpowiedzi obrazka.

Serwer buforuje pełny obraz w RAM i sprawdza długość oraz CRC-32/ISO-HDLC przed
przekazaniem jakichkolwiek danych odbiorcy. Następnie ponownie dzieli go na fragmenty.
Nie interpretuje formatu PNG/JPEG ani nie wykonuje dekompresji. CRC służy kontroli
spójności, nie kryptograficznemu uwierzytelnieniu. Nazwa jest wyłącznie metadaną;
ścieżki, separatory, znaki sterujące i nazwy `.`/`..` są odrzucane.

Stałe granice lokalnego profilu:

| Zasób | Limit |
|---|---|
| Pojedynczy obraz | 255 KiB i deklaracja odbiorcy |
| Aktywne transfery | 32 globalnie, 4 na konto w obu rolach |
| Rezerwacja danych | 8 MiB; rezerwacja rozmiaru przy żądaniu, alokacja po pierwszej części |
| Odpowiedź protokołu | 1909 B opcji, w tym 9 B nagłówka |
| Nazwa | 1–1024 B, dodatkowo mieści się we fragmencie |
| Liczba fragmentów przychodzących | 512 |
| Cały transfer wraz z wysłaniem | 30 s monotonicznie |
| Zapis jednego pakietu | istniejący `network.write_millis` |

Rezerwacja bufora nie obejmuje małych narzutów Vec/nazw/ramek; nie jest obietnicą
całkowitego RSS procesu. Przekroczenie długości, powtórzony początek, zła kolejność,
CRC, timeout lub zerwane połączenie kończy operację. Ponowne pobranie wymaga nowego
żądania. Nie ma trwałego cache ani pobierania w tle podczas nieobecności odbiorcy.

Bramka odczytu jest trzymana do zakończenia konkretnego zapisu TCP, nigdy przez
całe 30 s. W tym czasie nie oczekuje się na aktora konta. Limit czasu zapisu
obejmuje pobranie ze storage. Reset procesu usuwa bufory, zachowuje oferty.

## Dowody

`image_tests.rs`: atomowość, TTL, niezależność od historii, quota ofert, blokady,
retirement, Ready, generacje i oczekiwanie zapisu polityki na publikację.
`rgg-transfers`: CRC, kolejność, przepełnienie, 512 części, globalna/per-user pula,
deadline i anulowanie. `gg6_images.py`: rzeczywiste TCP, restart, NO_ACK, CRC,
obcy odbiorca, blokada i zastąpienie sesji, ukryta obecność, limit klienta, 255 KiB,
konferencja, awarie przed/po commit oraz migracja rzeczywistego dev.9.
