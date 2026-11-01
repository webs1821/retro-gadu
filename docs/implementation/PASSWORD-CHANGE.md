# Lokalna zmiana hasła — dev.16

Komenda offline wymaga aktualnego hasła. To uwierzytelniona zmiana, bez resetu
przez niezweryfikowany e-mail lub pominięcia starego hasła. Działający serwer
posiada blokadę katalogu i uniemożliwia uruchomienie CLI na tych samych danych.
Implementacja dotyczy nowego rGG, nie zmienia działającego gg-retro użytkownika.

```sh
python tools/change_password.py --admin target/debug/rgg-admin \
  --data-dir /private/rgg-data --uin 12345
target/debug/rgg-admin password-audit --data-dir /private/rgg-data
```

Pomocnik wymaga terminala z ukrytym wpisywaniem i dwukrotnie pyta o nowe hasło.
Nie umieszcza żadnego hasła w argumentach ani pliku. Bezpośrednie
`rgg-admin password-change --data-dir KATALOG --uin NUMER` przyjmuje na stdin
JSON z dokładnie `current_password` i `new_password`, do 4096 B. Błędny JSON
ma stały komunikat bez jego zawartości. Polityka obu haseł: 8–15 drukowalnych
znaków ASCII bez spacji, tak jak dotychczasowe konta GG6.

## Transakcja i uwierzytelnienie

Katalog musi być zainicjalizowany i mieć oryginalny keyring. Niedokończona
rotacja `keys.pending` wymaga najpierw wznowienia. Operacja nie regeneruje
kluczy, nie odblokowuje konta i nie tworzy brakującego konta.

Pod rezerwacją zapisu SQLite sprawdzane są: brak aktywnej sesji tego konta,
aktywność konta, AEAD jego sekretu i pełne obecne hasło. Porównanie używa
`subtle::ConstantTimeEq` na dwóch 16-bajtowych buforach z długością; nie używa
GG32 jako weryfikatora hasła. Te bufory i hasła w Rust są zeroizowane.
Python nie daje gwarancji wymazania pamięci swoich obiektów.

Nowe hasło musi być inne. `credential_version` rośnie o 1 z kontrolą
przepełnienia. Świeży szyfrogram, rezerwacja pary klucz/nonce i wpis audytu
są jednym commit. Kolizja nonce lub błąd wycofują całość. Zmiana nie zużywa
zaproszenia i zachowuje UIN, e-mail, prywatność, profil, archiwum kontaktów,
wiadomości i prawa do obrazków. Wersja uprawnień odrzuca wcześniejszy dowód
logowania przy przyszłym attach sesji. Klucze szyfrowania pozostają te same.

API repozytorium jest nazwane `change_password_offline`; nie jest gotową
operacją online. Taka operacja wymaga dodatkowo zatrzymania/fencingu socketów
przed zmianą uprawnień, zgodnie z istniejącym kontraktem sesji. Brak endpointu
WWW ani legacy HTTP zmiany hasła w tym przyroście.

## Audyt i kopie

Schemat 11 dodaje `admin_audit`. Udane zmiany zapisują kanał
`local_cli_with_current_password`, rodzaj operacji, UIN, czas oraz poprzednią
i nową wersję uprawnień. Kanał nie jest deklaracją tożsamości konkretnego
użytkownika systemu. Nie zapisujemy hasła, skrótu GG32, e-maila, szyfrogramu
ani treści błędnego żądania. To dziennik zatwierdzonych zmian, nie wszystkich
odrzuconych wywołań. Rekord nie znika automatycznie po usunięciu konta.

Limit fizyczny to 10000 wpisów, przycinany w transakcji zmiany. Wpisy starsze
niż 30 dni są usuwane przy otwarciu bazy i kolejnej zmianie; odczyt dodatkowo
filtruje je po czasie. `password-audit` zwraca najwyżej 100 najnowszych wpisów.
Nie ma osobnego harmonogramu czyszczenia wyłączonej/bezczynnej instalacji.
Historyczne kopie zachowują swój stan, także dawny audyt i dawne hasło.

Kopia sprzed zmiany odtwarza poprzednie hasło, a kopia po zmianie — nowe.
Obie zachowują kompletne, niezależnie działające dane. Przetestowano tę
właściwość przez rzeczywiste logowania GG6. Ogólne reguły [BACKUPS](BACKUPS.md)
pozostają w mocy; odtworzenie starszej kopii wymaga odpowiedniej wersji narzędzia.

## Weryfikacja

Trzy nowe testy Rust: autoryzacja i rollback nonce, aktywna sesja/wyłączone
konto/przepełnienie wersji, oraz limit/retencja audytu. `tests/password_cli.py`
sprawdza nowe i stare logowanie GG6, kolejkę z obrazkiem, profil i kontakty,
odtworzenie obu kopii, brak jawnych haseł w bazie/wynikach, błędne wejście,
nieistniejące/usunięte konta, brak klucza, pending rotation i blokadę procesu.
Awaria przed commit zachowuje stare hasło; po commit nowe. Wersja, nonce
i audyt są zgodne w obu przypadkach. Końcowy build nie zawiera test-hooks.

Sprawdzono też rzeczywistą migrację dev.15 (schemat 10) do 11 oraz odtworzenie
starej kopii przez dev.15 i późniejszą zmianę hasła dev.16. Linux/WSL, wyłącznie
dane syntetyczne; oryginalny GG6 i natywny Windows nadal niekwalifikowane.
