# Kopie i odtwarzanie — dev.12

Wyłącznie lokalne CLI. Serwer musi być zatrzymany: proces administratora bierze
tę samą blokadę systemową co serwer. Nie ma opcji jej pominięcia. Źródło musi być
zainicjalizowane i posiadać keyring. Kopia samego pliku bazy nie jest kompletem.

```sh
rgg-admin backup-create --data-dir /srv/rgg/data --output /srv/rgg/backups/2026-11-01
rgg-admin backup-verify --backup /srv/rgg/backups/2026-11-01
rgg-admin restore --backup /srv/rgg/backups/2026-11-01 --data-dir /srv/rgg/restored
```

Przykłady nie są wykonywane na serwerach użytkownika. Rodzic ścieżki wyjściowej
musi istnieć; katalog kopii/docelowy **nie może istnieć**. Nie ma overwrite.
Po odtworzeniu operator wskazuje nowy katalog w lokalnej konfiguracji, a stary
zachowuje do czasu zakończenia prób. Nie usuwać ręcznie znacznika niepełnego restore;
powtórzyć odtwarzanie do innego, nieistniejącego katalogu.

## Zawartość i weryfikacja

- `rgg.sqlite`: spójny obraz SQLite przez `VACUUM INTO`, maksymalnie 2 GiB;
  nie kopiujemy osobno WAL/SHM. Zachowane są m.in. kolejka, oferty obrazków,
  profile, kontakty, zaproszenia, historia nonce i rezerwacje usuniętych numerów.
- `keys.toml`: pełny keyring, także starsze klucze odczytu, do 16 KiB.
- `manifest.json`: format 1, wersja programu, schemat, liczba istniejących kont,
  wymagane key_id oraz długość/SHA-256 obu plików, do 16 KiB. Nie ma w nim ścieżek.

Kontrola otwiera bazę tylko do odczytu (`immutable`, `trusted_schema=OFF`),
sprawdza `integrity_check`, klucze obce, wszystkie numery/checksumy migracji
oraz odszyfrowanie sekretu **każdego** konta. Nie drukuje sekretów.
Samo sprawdzenie pierwszego rekordu dla key_id nie wystarcza.
Weryfikacja nie uruchamia recovery epoki, migracji ani zapisu w kopii.

Odrzucane są dodatkowe pliki (także WAL i manifest.pending), symlinki oraz
na Unix uprawnienia dostępne grupie/innym. Nowy katalog ma 0700, pliki 0600.
Hashy nie traktujemy jako podpisu: osoba mająca klucze i zapis do kopii może
przepisać również manifest. Wejściem są zaufane kopie operatora, nie uploady
nieznanych baz SQLite. Keyring i baza razem pozwalają odzyskać hasła.

## Przerwanie operacji

Manifest jest publikowany ostatni, przez rename po fsync plików. Katalog i
jego rodzic są synchronizowane. Niedokończona kopia pozostaje prywatna, ale
nie przejdzie verify. Nie kasujemy automatycznie materiału po błędzie.

Restore najpierw sprawdza komplet, tworzy nowy katalog z blokadą i trwałym
`restore-incomplete`, kopiuje pliki przez create_new, porównuje hashe ponownie
i ponownie weryfikuje bazę oraz wszystkie konta. Dopiero wtedy usuwa znacznik
i synchronizuje katalogi. Serwer i zwykłe komendy admina odmawiają otwarcia
katalogu z tym znacznikiem. Pierwszy poprawny start po restore tworzy nową
epokę, unieważnia stare sesje i odzyskuje niedokończone doręczenia.

Testy process-exit przed publikacją manifestu i usunięciem znacznika są dostępne
tylko przez feature `rgg-admin/test-hooks`; zwykły build nie reaguje na te zmienne.
To dowód zachowania procesu/SQLite, nie certyfikacja fizycznego zaniku zasilania.

## Granice tej wersji

Weryfikator przyjmuje dokładnie bieżący schemat (8 w dev.12/13, **9 od dev.14**)
i jego osadzone migracje. Przetestowana ścieżka starszej kopii: restore narzędziem
dev.13 do nowego katalogu, następnie otwarcie tego katalogu dev.14 i migracja 8 → 9.
Nie konwertuje historycznych kopii w miejscu; procedura przyszłych aktualizacji
wymaga osobnego testu i rozszerzenia reguły zgodności. Nie ma kopii na żywo,
szyfrowania całego archiwum, uploadu, retencji, automatycznego pruning ani
deduplikacji. Konfigurację, manifest pobierania i pliki klientów należy zachować
oddzielnie; nie są stanem SQLite i nie są tu automatycznie dołączane.

Sprawdzone na Linux/WSL i lokalnym filesystemie. Windows ACL, sieciowy filesystem,
duże rzeczywiste dane i procedura operatorska na Containers pozostają niekwalifikowane.
Implementacja snapshotu opiera się na dokumentacji
[SQLite VACUUM INTO](https://www.sqlite.org/lang_vacuum.html#vacuum_with_an_into_clause)
i [integrity_check](https://www.sqlite.org/pragma.html#pragma_integrity_check),
bez kopiowania kodu z zewnętrznego serwera.
