# Zachowanie zaproszeń gg-retro — dev.14, schemat 9

Nowa, osobna tabela `legacy_invitations` zachowuje stary SHA-256, limit trzech
użyć, dotychczasowe zużycie oraz active. Migracja 8 → 9 tworzy ją pustą.
Właściwy importer jeszcze jej nie wypełnia; testy robią to wyłącznie na danych
syntetycznych. Zwykłe generowanie zaproszeń rGG nadal produkuje własne kody.

Stary format to 24–128 znaków ASCII (litery, cyfry, `-`, `_`), z rozróżnianiem
wielkich i małych liter. Hash powstaje z dokładnych bajtów bez prefiksu.
Format rGG pozostaje 32 znakami hex, bez rozróżniania wielkości liter, z własnym
prefiksem przy haszowaniu. Nie konwertujemy hashy bez znajomości kodów ani nie
przechowujemy kodów jawnie w bazie.

WWW i CLI przekazują opcjonalne kandydaty obu rodzajów. Repozytorium najpierw
próbuje kodu rGG; jeżeli taki digest istnieje, jego stan jest rozstrzygający.
Wyczerpany kod rGG **nie przechodzi** do starej tabeli po dodatkowe użycia.
Tylko przy braku tego digestu można zużyć aktywne, niewyczerpane stare zaproszenie.
Transakcja obejmuje zużycie, przydział numeru, szyfrowanie i utworzenie konta.
Kolizja numeru lub inny błąd wycofuje także naliczone użycie starego kodu.

Formularz przyjmuje do 128 znaków. Limity żądań, CSRF, Host/Origin i brak
logowania kodów pozostają takie jak w rejestracji rGG. Adapter hashujący używa
standardowego SHA-256 według faktów odczytanych z przypiętego forka; nie kopiuje
implementacji rejestracji z gg-retro. Pochodzenie w THIRD_PARTY.

Backup/restore obejmuje tabelę razem z bazą. Starsze kopie o schemacie 8 należy
odtworzyć narzędziem dev.13 do nowego katalogu, potem otworzyć ten katalog dev.14,
co wykona migrację. Nowy weryfikator kopii wymaga schematu 9 i odrzuca starszą
kopię bez modyfikacji. Ta ścieżka została sprawdzona na zachowanych binarkach.

Od dev.15 [importer całego forka](IMPORT-GG-RETRO.md) wypełnia tę tabelę w tej samej
transakcji co konta i pozostałe dane. Bieżący schemat to 11; procedury kopii
opisuje [BACKUPS](BACKUPS.md). Nie ma synchronizacji pliku jawnych kodów ani
publicznej operacji administracyjnej.
