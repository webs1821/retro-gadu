# Rotacja kluczy — dev.13

Offline CLI pod tą samą blokadą katalogu co serwer. To ponowne szyfrowanie
sekretów kont; **hasła użytkowników i credential_version nie zmieniają się**.
Nie jest to reset hasła ani operacja legacy HTTP. Żadne prawdziwe dane nie były
zmieniane podczas implementacji.

```sh
rgg-admin keys-status --data-dir /srv/rgg/data
rgg-admin keys-rotate --data-dir /srv/rgg/data --backup /srv/rgg/backups/before-rotation
rgg-admin keys-resume --data-dir /srv/rgg/data
```

`keys-status` sprawdza odszyfrowanie wszystkich kont, podaje aktywny key_id,
listę zachowanych identyfikatorów, liczby kont dla każdego klucza, liczbę
rekordów do przepisania i obecność keys.pending. Nie ujawnia kluczy ani haseł.
Każda komenda wymaga wcześniej zainicjalizowanego katalogu i zatrzymanego serwera.

## Kolejność zapewniająca możliwość odzyskania

1. Sprawdzamy wszystkie konta, także nieaktywne. Nowa rotacja jest odrzucana,
   jeśli poprzednia ma pending file lub część rekordów używa starszych kluczy.
2. `keys-rotate` wymaga pełnego [backupu](BACKUPS.md) do nowej ścieżki. Błąd kopii
   zatrzymuje operację przed zmianą keyringu lub zaszyfrowanych rekordów.
3. CSPRNG tworzy klucz 32 B i identyfikator 12 losowych bajtów. Nowy keyring
   zachowuje wszystkie poprzednie klucze; maksimum to 32. Limit odmawia dalszej
   rotacji, nigdy nie usuwa automatycznie najstarszego klucza.
4. Pełny keyring trafia do nowego pliku keys.pending (0600, create_new, fsync),
   następnie zastępuje keys.toml przez rename w tym samym katalogu i fsync katalogu.
   Dopiero po trwałej publikacji klucza można zapisywać zaszyfrowane nim rekordy.
5. Partie najwyżej 64 kont są odczytywane, odszyfrowywane i szyfrowane nowym
   nonce. Rezerwacja pary key_id/nonce oraz zmiana rekordu należą do jednej
   transakcji. Kolizja nonce wycofuje całą partię; wcześniejsze zatwierdzone
   partie pozostają. Historia nonce nigdy nie jest usuwana przez rotację.
6. Po zakończeniu ponownie sprawdzamy wszystkie konta i raportujemy liczbę
   przepisanych rekordów oraz pozostałych. Komenda nie wypisuje materiału kluczy.

`keys-resume` publikuje poprawny pending keyring tylko wtedy, gdy jest on
następcą zachowującym dokładnie wszystkie stare klucze i dodającym jeden nowy
aktywny klucz. Inny lub uszkodzony plik jest odrzucany, bez zastąpienia keys.toml.
Po publikacji wznawiane są tylko rekordy nadal używające starego klucza.
Wznowienie zakończonej rotacji jest no-op; `keys-rotate` rozpoczyna nową rotację.

Po przerwaniu między partiami aktywny keyring nadal odszyfrowuje wszystkie
rekordy. Można się zalogować, ale operacyjnie należy najpierw dokończyć rotację.
Backupu sprzed rotacji nie podmieniamy: zawiera własny, pasujący keyring.

## Granice i odzyskiwanie po błędzie

Testy obejmują przerwanie po trwałym zapisie keys.pending, po publikacji klucza
oraz po pierwszej zatwierdzonej partii. Proces z test-hooks kończy się kodem 86;
zwykły build nie odczytuje zmiennej fault injection.

Przerwanie w połowie zapisu keys.pending może pozostawić niepoprawny plik.
Wtedy resume odmawia automatycznej naprawy; oryginalny keyring i rekordy nie
były jeszcze zmienione. Nie nadpisywać keys.toml ani nie usuwać kluczy na próbę.
Pełna kopia sprzed rotacji pozwala odtworzyć stan do nowego katalogu. Narzędzie
nie udaje odporności na uszkodzony filesystem, równoczesne zewnętrzne zapisy
operatora, utratę całego dysku ani utratę obu kopii kluczy.

Nie ma jeszcze migracji po wyczerpaniu limitu 32 zachowanych kluczy, automatycznej
retencji ani rotacji online. Linux/WSL sprawdzony; Windows ACL i fizyczny zanik
zasilania pozostają niekwalifikowane. Semantyka rename według
[oficjalnej dokumentacji Rust](https://doc.rust-lang.org/std/fs/fn.rename.html).
