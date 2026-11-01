# Import gg-retro — dev.15

Lokalny importer przenosi **spójny, nieaktywny obraz** przypiętego gg-retro 0.8.3
do nowego katalogu rGG (schemat 10). Nie łączy się z produkcją, nie zmienia źródła,
nie nadpisuje celu i nie wypisuje haseł. Testy używają wyłącznie danych syntetycznych.

## Przygotowanie i komendy

Potrzebne są: samodzielny snapshot SQLite, oryginalny plik klucza ggv1
(32 surowe bajty) oraz JSON z dodatkowymi rezerwacjami numerów. Źródło musi być
zatrzymane albo snapshot wykonany spójną operacją SQLite, np. VACUUM INTO.
Nie wskazywać aktywnej bazy ani kopiować osobno plików WAL. Zachować prywatną
kopię oryginalnych danych, klucza i konfiguracji poza Git.

```sh
python tools/inspect_gg_retro.py --source /private/snapshot/gg.sqlite
python tools/migrate_gg_retro.py --source /private/snapshot/gg.sqlite \
  --legacy-key /private/snapshot/password.key --reserved-uins /private/reserved.json \
  --acknowledge-missing-history --admin target/debug/rgg-admin --check
python tools/migrate_gg_retro.py --source /private/snapshot/gg.sqlite \
  --legacy-key /private/snapshot/password.key --reserved-uins /private/reserved.json \
  --acknowledge-missing-history --admin target/debug/rgg-admin --data-dir /private/rgg-new
```

To przykłady operatorskie, nie wykonane polecenia na serwerach użytkownika.
`reserved.json` to tablica liczb, np. `[12345, 54321]`; wolno świadomie przekazać
`[]`. W gg-retro nie ma kompletnej historii usuniętych numerów ani wszystkich
dawnych ustawień prywatności. Obowiązkowa opcja `--acknowledge-missing-history`
potwierdza tę lukę; importer nie wymyśla brakujących danych. Raport wskazuje
liczbę kont bez zapisanej prywatności. Nie publikuje sesji online, a połączenie
GG nadal wymaga bieżącej synchronizacji reguł przed doręczeniem wiadomości.

Sam inspector pozostaje narzędziem strukturalnym: `migration_ready: false`
oznacza, że nie weryfikuje haseł i nie wykonuje konwersji. Dopiero `--check`
uruchamia pełny walidator Rust i AEAD wszystkich kont. Nie tworzy katalogu celu.
Poprawny check nie rezerwuje zasobów ani nie zastępuje kontroli podczas apply.

## Mapowanie i jawna polityka

| Dane gg-retro | Zachowanie rGG |
|---|---|
| UIN, e-mail, data utworzenia | Zachowane; zakres GG6 i format e-maila sprawdzone. Brak daty oznacza czas importu w koncie, a oryginalny NULL zostaje w prywatnych metadanych. |
| Nazwa konta i jego zaproszenie | Prywatne metadane; nie stają się publicznym profilem. |
| Hasło ggv1 | Zweryfikowane z oryginalnym kluczem i AAD UIN, ponownie zaszyfrowane świeżym keyringiem rGG. Brak plaintext fallback. Hasło użytkownika nie zmienia się. |
| Stare zaproszenia | Hash SHA-256, max_uses, used i active zachowane w osobnej tabeli; brak resetu użyć. |
| next_uin | Zachowany niezależnie od numerów wybranych ręcznie. Dodatkowe rezerwacje stają się retired_uins. |
| Archiwa kontaktów | Konwersja CP1250 bez utraty znaków, osobno od reguł prywatności; import jako wersja 1. |
| Offline presence | Opis, powrót, tylko znajomi i zapisane reguły kontaktów; nie odtwarzamy obecności online. |
| Profile publiczne | Tylko jawne rekordy profilu. Polskie klucze wyszukiwania przeliczone; poprawne mapowanie płci źródła. |
| Pending messages | Identyfikatory, FIFO, seq, czas, tekst, formatowanie i konferencja zachowane. Nadawca i odbiorca muszą istnieć. |
| Delivered messages | Tylko końcowy nagłówek TransportWrittenUnconfirmed; bez tekstu/stylów/konferencji i bez ponownego kolejkowania. To nie jest ACK odbiorcy. |
| Oferty obrazków | Dokładna zgodność deskryptorów z size/CRC i odbiorcą. Brak trwałych bajtów obrazka i dodatkowych uprawnień uczestników konferencji. |
| Transient tokens | Pomijane i liczone w raporcie; nie stają się aktywnymi uprawnieniami. |

Pending dostają **nowy TTL od chwili importu**: domyślnie 30 dni, opcja
`--pending-ttl-seconds` przyjmuje 1–2592000 sekund. Czas samej wiadomości zostaje
oryginalny. Oferty już wysłanych obrazków dostają 7 dni od importu, a oczekujących
— TTL wiadomości. Obie decyzje retencji są zapisane w raporcie. Historia
wiadomości wysłanych nie jest importowana jako archiwum treści.

Nieznane/osierocone konta, sprzeczność oczekującej treści lub oferty z zapisaną
blokadą, duplikaty, niezgodne opcje wire, niewspierane znaki lub przekroczenia
limitów powodują odmowę całości. Importer niczego takiego po cichu nie odrzuca.
Numery z terminalnych nagłówków mogą wskazywać jawnie podane rezerwacje.

## Trwałość i granice wejścia

Nowy katalog ma na Unix 0700, pliki klucza/znacznika 0600. Znacznik
`import-incomplete` jest synchronizowany **przed utworzeniem SQLite**.
Specjalne otwarcie nowego importu przyjmuje tylko znacznik i plik blokady,
odmawiając istniejącej bazy lub keyringu. Zwykły serwer/admin odmawia katalogu
ze znacznikiem, również po zatwierdzeniu danych i przerwaniu końcowej weryfikacji.

Wszystkie dane są zapisywane w jednej transakcji. Receipt zawiera SHA-256 źródła
i pakietu wymiany, czas i liczniki/politykę. Przed commit są sprawdzane kwoty,
integralność i FK; po commit odszyfrowanie wszystkich nowych kont i ich liczba.
Dopiero potem usuwany jest znacznik i synchronizowane katalogi. Awaria zostawia
prywatny, nieaktywny cel. Powtórzyć do nowego katalogu; nie usuwać znacznika ręcznie.

Źródło: zwykły plik do 2 GiB, bez symlinków i WAL/SHM/journal, na Unix tylko dla
właściciela. Przypięte 9 tabel, 11 migracji i ich checksumy; odmowa dodatkowych
tabel, kolumn, widoków i triggerów. Odczyt jest immutable/read-only/query_only,
z trusted_schema=OFF. Przed i po odczycie sprawdzane są hash i stan pliku.
Brak sidecarów sam nie dowodzi braku writera — źródłem ma być nieaktywny snapshot.

Inspekcja SQL ma limit 30 s, eksport 60 s, pola i liczby rekordów są ograniczone.
Pakiet JSON do 64 MiB zawiera stare **szyfrogramy**, nigdy odszyfrowane hasła,
i trafia przez stdin do Rust. Python nie odczytuje pliku klucza. Apply wymaga
nowego katalogu; wszystkie walidacje logiczne i stare AEAD poprzedzają jego
utworzenie. Żaden komunikat błędu nie zawiera wiersza źródła.

Limity: do 10000 kont/kodów/archiwów/profili, 100000 nagłówków i rezerwacji,
80000 ofert; standardowe kwoty kolejki (1000 na nadawcę/odbiorcę, 10000 globalnie,
4 MiB na odbiorcę i 32 MiB globalnie) i limit ofert również obowiązują.
Nie jest to sandbox nieznanych baz SQLite: wejściem jest zaufana kopia operatora.

## Pochodzenie i weryfikacja

Referencja: webs1821/gg-retro 0.8.3, commit
`ed9640cc247764754b5e47891053eb07a53f76d5`. Własne implementacje czytnika,
walidacji i zapisu powstały na podstawie faktów formatów, bez kopiowania kodu
serwera o niestandardowej licencji MIS. [Referencja schematu](gg-retro-schema-reference.json)
zawiera fakty kolumn i sumy przypiętych plików Git (LF), nie kopie migracji.
Wektory ggv1 powstały niezależnie przez API systemowej libsodium 1.0.18,
według [formatu XChaCha20-Poly1305](https://doc.libsodium.org/secret-key_cryptography/aead/chacha20-poly1305/xchacha20-poly1305_construction).
Klucze i hasła fixture są publicznymi danymi syntetycznymi. Pochodzenie w THIRD_PARTY.

`tests/legacy_inspection.py`: Windows 3.13 i Linux/WSL 3.8; read-only, brak PII
w wynikach, odmowa błędnego schematu/migracji/sidecarów/rozmiaru/zmiany źródła.
`tests/legacy_import.py`: Linux/WSL, rzeczywiste CLI/SQLite/TCP, logowanie po
przeszyfrowaniu, FIFO i obrazek, polski katalog, kontakty, zaproszenia, rezerwacje,
backup/restore i niezmienność źródła. Trzy kontrolowane przerwania procesu,
odmowa niekompletnych celów i ponowienie do nowego. Osobno migracja rzeczywistych
binarek dev.14 (schema 9 → 10) i restore starej kopii przez dev.14.

[Raport dev.15](VERIFICATION-2026-11-01-dev15.md). Nie wykonywano importu prawdziwej
bazy użytkownika, kwalifikacji natywnego GG6 ani wdrożenia.
