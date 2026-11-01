# Import gg-retro — inwentaryzacja i kontrola przed importem

Stan 2026-11-01: działa **inspekcja źródła**, przeniesienie do rGG jeszcze nie
jest zaimplementowane. Narzędzie zawsze raportuje `migration_ready: false`.
Nie przyjmuje klucza, nie odszyfrowuje ani nie wypisuje haseł i nie otwiera celu.

```sh
python tools/inspect_gg_retro.py --source /private/snapshot/gg.sqlite
```

Przygotować spójny **nieaktywny, samodzielny** obraz SQLite (np. VACUUM INTO)
z zachowaniem kompletnej prywatnej kopii źródła, klucza i konfiguracji poza Git.
Plik ma być regularny, do 2 GiB, bez WAL/SHM/journal, na Unix dostępny tylko
właścicielowi. Nie wskazywać działającej bazy. Brak sidecarów sam w sobie nie
udowadnia braku writera; kontrola zmian pliku to dodatkowe wykrywanie pomyłek.

Sprawdzane są przypięte tabele i kolumny, 11 wersji i SHA-384 migracji, integralność
oraz relacje. Nieznane schematy, widoki i triggery są odrzucane. Odczyt SQLite
używa `mode=ro`, `immutable`, `query_only` i wyłączonego zaufania do schematu.
Przed i po odczycie porównywane są SHA-256 i stan pliku. Zapytania mają limit
czasu; odczyty metadanych i rozmiary plików są ograniczone.

Raport zawiera tylko liczniki, stan alokatora i kody problemów. Prefix ggv1 nie
jest dowodem poprawności sekretu: raport jawnie zaznacza brak weryfikacji AEAD
i polityki haseł. Kod wyjścia 0 oznacza ukończenie **inspekcji**, nie zgodę na
migrację; problemy danych są w `findings`, a prace do wykonania w `required_before_import`.
Kod 2 oznacza odmowę źródła. Nie jest to sandbox dla dowolnych plików SQLite;
wejściem jest zaufana prywatna kopia operatora.

## Przypięte źródło i pochodzenie

Fork webs1821/gg-retro 0.8.3, commit
`ed9640cc247764754b5e47891053eb07a53f76d5`. Lokalny checkout był czysty.
Inwentaryzowano migracje i formaty w kodzie, bez odczytu produkcyjnej bazy.
[Referencja JSON](gg-retro-schema-reference.json) zapisuje fakty o 9 tabelach,
kolumnach oraz sumy plików migracji z przypiętych blobów Git (LF).
Nie zawiera kopii migracji ani implementacji źródła. Testy budują nowy syntetyczny
schemat z tych faktów, bez danych użytkowników. Licencja projektu źródłowego
jest niestandardowa MIS; użycie referencyjne jest odnotowane w THIRD_PARTY.

## Mapowanie wymagane przed właściwym importem

| Źródło | Docelowa semantyka i wymagane sprawdzenie |
|---|---|
| users.uin, email, created_at | Zachować UIN i datę; sprawdzić zakres GG6 i e-mail. Nie publikować nazwy/e-maila w katalogu. |
| users.password | `ggv1:` + base64(nonce 24 B + ciphertext/tag XChaCha20-Poly1305), AAD = UIN u32 little endian; oryginalny klucz to osobny plik 32 surowych bajtów. Zweryfikować wszystkie sekrety, następnie zaszyfrować keyringiem rGG. Bez plaintext fallback. |
| registration_invites | SHA-256 dokładnych, case-sensitive kodów ASCII 24–128 znaków; rGG obecnie stosuje inny format i domain-separated digest. Potrzebny adapter, zachowanie used/max_uses/active bez refundowania. Z samych hashy nie można obliczyć nowego rodzaju hasha. |
| registration_state.next_uin | Zachować licznik niezależny od ręcznych numerów; nie wyliczać max(UIN)+1. |
| contact_lists.data | Zweryfikować kodowanie i limity; importować osobno od reguł prywatności. |
| offline_presence | Zachować jawny opis offline/czas powrotu i zakodowane reguły prywatności. Nie odtwarzać stanu online ani niewidoczności. Nie każda historyczna lista blokad jest w tej tabeli. |
| public_profiles | Tylko jawnie zapisane profile; przebudować klucze wyszukiwania z pól, sprawdzić rok/płeć/tekst. |
| messages z delivered_at=NULL | Zachować id/mapowanie, kolejność, czas, nadawcę, odbiorcę, tekst, formatowanie i konferencję. Sprawdzić osierocone numery oraz politykę TTL przed przeniesieniem. |
| messages z delivered_at!=NULL | To zapis do transportu, nie ACK odbiorcy. Nie wstawiać ponownie do kolejki. Polityka historii musi być jawna. |
| message_images | Prywatne oferty size/CRC dla danej wiadomości i odbiorcy; nie ma trwałych bajtów obrazka. Zachować powiązanie, bez nadawania prawa innym uczestnikom konferencji. |
| tokens | Stare sesje CAPTCHA/rejestracji nie są przenośnym uwierzytelnieniem rGG. Nie odtwarzać jako aktywne uprawnienia. |
| Brak tabeli usuniętych UIN | Nie można odtworzyć nieistniejącej historii. Potrzebna dodatkowa lista rezerwacji albo jawna decyzja operatora przed włączeniem rejestracji. |

Właściwy import będzie wymagał nowego, pustego katalogu, trwałego znacznika
niepełnej operacji, pełnej transakcji albo jednoznacznego stagingu i manifestu
identyfikującego źródło. Ponowienie nie może duplikować danych. Ta wersja nie
udaje pełnego dry-run konwersji: nie dekoduje payloadów i nie tworzy kont.

## Weryfikacja

`tests/legacy_inspection.py` przeszedł na Windows/Python 3.13 i Linux/WSL/Python 3.8:
liczniki pending/delivered, pozostałe użycia zaproszeń, zachowanie alokatora,
niezmienność pliku oraz brak nazw, e-maili, haseł i treści w stdout/stderr.
Sprawdzono wykrycie równoczesnej zmiany, odmowę nowej kolumny/tabeli/triggera,
zmienionej migracji/checksumy, WAL/journal i przekroczenia rozmiaru. Linux
sprawdził również symlink i uprawnienia; te dwa przypadki pomijane są na Windows.
Nie użyto prawdziwej bazy ani klucza, nie dotknięto działającej usługi.
