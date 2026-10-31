# Niezależny przegląd planu Retro Gadu: bezpieczeństwo, testy i operacje

Data: 2026-10-31. Przegląd dokumentu w wersji 0.1, nie implementacji.

Przeczytany dokument: `D:\Dokumenty\Documents\RetroGadu\PLAN_IMPLEMENTACJI.md`.
SHA-256: `380d006a7244b65d1e9f4c38365c0f99f3b75acb5b5e6f7cbe54b74b2826d67b`.

Zakres: tożsamość i uprawnienia, prywatność, klucze, nadużycia, WWW, pochodzenie kodu, testy, kopie, migracja i powrót po wdrożeniu. Nie zmieniano planu ani kodu, nie łączono się z produkcją, nie wykonano testów działającego rGG. Podane scenariusze wskazują możliwe skutki niedookreślonego kontraktu, a nie potwierdzone podatności istniejącego programu.

## Ocena i znaczenie wag

Plan ma dobre podstawy: wspólną politykę prywatności, fencing sesji i dzierżaw, ograniczenia parsera, trwały zapis przed potwierdzeniem, uczciwy model legacy bez ACK, atomową rejestrację, spójny backup SQLite i zakaz bezwarunkowego cofnięcia bazy po nowych zapisach. Rozdział licencyjny prawidłowo odróżnia MIS od MIT i port implementacji od samodzielnego kodu.

Nie stwierdzam P0 na poziomie tego dokumentu. P1 poniżej oznacza warunek doprecyzowania i sprawdzenia przed wskazaną funkcją lub wdrożeniem; nie blokuje całych lokalnych prac E01. P2 to usprawnienie specyfikacji lub dowodów. Wystarczy rozliczenie uwagi konkretną decyzją, zmianą kontraktu i wskazaniem testu; nie każda wymaga osobnego modułu.

| ID | Waga | Charakter / bramka | Temat |
| --- | --- | --- | --- |
| S01 | P1 | Blokuje uwierzytelnianie i odwoływanie uprawnień E02/E05 | Reset lub blokada kontra logowanie i komendy w toku |
| S02 | P1 | Blokuje publikację prywatności E04 | Zmiana polityki kontra zakolejkowane ramki |
| S03 | P1 | Blokuje publiczne usuwanie i ponowną rejestrację E05 | Tożsamość po usunięciu numeru |
| S04 | P1 | Blokuje przyjęcie prawdziwych haseł E02 i operacje E07 | Cykl życia kluczy i nonce |
| S05 | P1 | Blokuje publiczną samoobsługę/admin E05 | Reset, sesje WWW i granica dostępu administracyjnego |
| S06 | P1 | Blokuje publiczne E07 | Całościowy budżet zasobów i koszt uwierzytelniania |
| S07 | P1 | Blokuje deklarację trwałości E03/E07 | Awaria procesu a utrata zasilania i ustawienia SQLite |
| S08 | P1 | Blokuje operacyjne E07/E08 | Kompletny backup i bezpieczny restore |
| S09 | P1 | Blokuje przełączenie E08 | Wykonalny powrót po nowych zapisach |
| S10 | P1 | Blokuje tylko włączenie danego zapożyczenia/wydanie | Weryfikowalny rejestr pochodzenia |
| S11 | P2 | Sugestia do E01/E07 | Testowanie sekwencji protokołu i mierzalne bramki |
| S12 | P2 | Sugestia; zamknąć przed testami na kopii danych | Izolacja danych, egress i granice modelu zagrożeń |
| S13 | P2 | Sugestia do E03/E07 | Retencja treści i operacyjne alarmy |

## Uwagi

### S01 — P1: unieważnienie musi obejmować operacje już rozpoczęte

**Sekcja:** M07–M09, M18–M19, tabela `accounts.credential_version` w §5, §6.1, §9.3.

**Scenariusz ryzyka:** logowanie pobiera wersję hasła V, administrator blokuje konto albo użytkownik resetuje hasło, a stara weryfikacja kończy się później i dołącza sesję. Analogicznie komenda wysłana przez starą sesję może czekać w kolejce i wykonać się po jej odwołaniu. Samo usunięcie wpisu sesji lub zwiększenie `SessionGeneration` nie określa jeszcze atomowej relacji ze zmianą hasła i stanu konta.

**Konkretna poprawka:** określić punkty atomowego sprawdzenia. Reset/zmiana hasła, blokada i usunięcie zwiększają `credential_version` lub osobny równoważny numer odwołania; przy dołączeniu porównać wersję używaną podczas weryfikacji oraz bieżący stan konta. Koordynator konta porządkuje te zmiany z autoryzowanymi komendami. Każda odłożona komenda niesie tożsamość i generację, sprawdzane przed skutkiem domenowym. Reset unieważnia odpowiednie sesje GG i WWW oraz wcześniejsze tokeny resetu zgodnie z jedną opisaną polityką. Aktualizacja credentialów WWW i legacy musi być atomowa, jeśli reprezentują to samo hasło. Dla każdego stanu M09 zapisać dozwolone operacje; parser poprawnej ramki nie jest zgodą na jej wykonanie przed `Ready`.

**Kryterium odbioru:** deterministyczne testy zatrzymują operację po pobraniu starego credentialu, wykonują reset/blokadę, następnie wznawiają logowanie — sesja nie powstaje. Dwa równoczesne użycia resetu mają jednego zwycięzcę. Odłożona komenda starej generacji nie zmienia bazy. Pakiety wiadomości/katalogu/transferu przed uwierzytelnieniem, powtórny login i późniejsze pakiety innego profilu mają określony wynik bez skutku ubocznego.

### S02 — P1: ponowna kontrola prywatności nie zamyka sama wyścigu z writerem

**Sekcja:** I05, M10–M13, M15–M16, §6.2.

**Scenariusz ryzyka:** worker sprawdza uprawnienia, odkłada ramkę do wolnego writera, a następnie następuje blokada lub zmiana widoczności. Po potwierdzeniu zmiany writer wysyła wcześniej przygotowaną treść/status albo część obrazka. W planie są ponowne sprawdzenie i rewizja reguł, lecz nie ma granicy skuteczności zmiany względem buforów wyjścia.

**Konkretna poprawka:** zdefiniować liniową kolejność autoryzacji, zmian polityki i przekazania do writera, np. przez koordynator konta i rewizję polityki w metadanych oczekujących zdarzeń. Zmiana unieważnia niewysłane ramki/oferty objęte nową odmową; writer nie może bez sprawdzenia opróżnić starego bufora. Objąć tym wiadomości, statusy, konferencje, oferty i żądania obrazków. Jasno zapisać, że bajtów już przekazanych do TCP nie można odwołać; określić zachowanie rozpoczętej ramki/transferu. Nie trzymać transakcji SQL ani globalnego mutexa przez oczekiwanie na sieć.

**Kryterium odbioru:** test z kontrolowanym wolnym writerem ustawia blokadę między sprawdzeniem, kodowaniem i wysyłką. Zdarzenia pozostające po zdefiniowanej granicy nie wychodzą. Test odróżnia bajty już oddane TCP od kolejki aplikacji. Powtórzyć dla niewidoczności, offline, konferencji i autoryzowanego wcześniej obrazka. Udokumentować również decyzję, czy odblokowanie uwalnia wstrzymane wiadomości.

### S03 — P1: usunięty UIN nie może milcząco stać się nową osobą

**Sekcja:** M02, M07, M13, §5, §11.1.

**Scenariusz ryzyka:** po usunięciu konta ręczna rezerwacja ponownie przydziela ten sam numer. Stare kontakty i wiadomości innych użytkowników odnoszą się do numeru, więc nowy właściciel może odziedziczyć zaufanie, adresowanie nowych rozmów lub powiązane oferty. Monotoniczny licznik automatyczny nie zapobiega ręcznemu ponownemu przydziałowi.

**Konkretna poprawka:** dla pierwszej wersji przyjąć zasadę braku ponownego przydzielania usuniętego numeru i zachowywać minimalny tombstone UIN. Oddzielić minimalną rezerwację numeru od usuwanych danych osobowych. Import/eksport i backup muszą zachować te rezerwacje. Jeśli kiedykolwiek ma powstać odzyskiwanie numeru, wymaga osobnej specyfikacji tożsamości oraz widocznych skutków dla kontaktów i kolejki.

**Kryterium odbioru:** po usunięciu, restarcie i migracji rejestracja tego UIN zostaje odrzucona; usunięte konto nie odbiera wiadomości, ofert ani logowania. Test sprawdza zarówno przydział ręczny, jak i automatyczny oraz ponowny import.

### S04 — P1: AEAD i klucz poza bazą wymagają pełnego kontraktu operacyjnego

**Sekcja:** M08, M20–M21, `legacy_credentials` w §5, E02/E07, §11.2.

**Scenariusz ryzyka:** rotacja przerywa się w połowie i pozostawia rekordy pod dwoma kluczami, stary klucz zostaje usunięty, albo backup zawiera bazę bez pasujących kluczy. Alternatywnie współbieżne zapisy lub odtworzenie generatora powtarzają nonce dla tego samego klucza. Samo słowo „AEAD” ani `key_id` nie definiują wymaganej unikalności ani bezpiecznej rotacji.

**Konkretna poprawka:** wybrać konkretny format i sprawdzoną bibliotekę AEAD, sposób generowania nonce zgodny z jej wymaganiami, źródło losowości oraz wersjonowane AAD. Dopisać: błąd losowości lub brak/wadliwy klucz powodują odmowę, nigdy fallback do plaintext. Keyring rozróżnia klucz aktywny do nowych zapisów i klucze wyłącznie do odczytu; rotacja jest wznawialna i raportuje pozostałe rekordy. Usunięcie starego klucza uwzględnia także zachowane backupy. Pliki kluczy mają ograniczony dostęp, backup odrębny od bazy i manifest potrzebnych `key_id`. Zdefiniować zakres, w którym kompromitacja bazy, a w którym bazy wraz z kluczem ujawnia legacy sekret; szyfrowanie magazynowe nie wzmacnia starego protokołu w sieci.

**Kryterium odbioru:** przerwana rotacja wznawia się bez utraty dostępu do kont; odtworzenie mieszanego zestawu `key_id` działa na osobnej instancji. Zamiana ciphertext między kontami, uszkodzenie nonce/AAD oraz nieznany klucz skutkują odmową. Testy współbieżności i format repozytorium nonce nie dopuszczają ponownego użycia w wybranym schemacie. Logi i błędy nie zawierają danych wejściowych kryptografii.

### S05 — P1: WWW potrzebuje kontraktu odzyskiwania konta i sesji, nie tylko cookie flags

**Sekcja:** M07–M08, M18–M19, `web_sessions`, `reset_tokens`, `email_outbox` w §5, E05.

**Scenariusz ryzyka:** reset kierowany jest na niepotwierdzony albo właśnie zmieniany e-mail; stary token działa po zmianie credentialu; sesja przed zalogowaniem zostaje zachowana po loginie. Payload trwałego outboxa zawiera aktywny token resetu mimo zapisania wyłącznie jego skrótu w `reset_tokens`. Admin ograniczony tylko regułą reverse proxy pozostaje dostępny przez bezpośrednio opublikowany port.

**Konkretna poprawka:** określić, które zweryfikowane kanały pozwalają odzyskać konto i jak zmienia się adres odzyskiwania. Powiązać reset z kontem, celem i wersją credentialu; zużycie tokenu i zmiana hasła stanowią jedną transakcję. Strona resetu używa skonfigurowanego originu, nie Host/X-Forwarded-Host; odpowiedzi nie ujawniają istnienia e-maila/konta. Jeśli link resetu jest tokenem bearer, opisać jego kontrolowaną obecność w URL, redakcję logów proxy, brak zasobów stron trzecich i politykę referrera — obecny ogólny zakaz sekretów w URL nie określa wykonalnego transportu linku. Outbox zawierający token traktować jak sekret: ograniczony dostęp, szyfrowanie w spoczynku według modelu zagrożeń, szybkie usuwanie po wysyłce/wygaśnięciu. Sesje WWW mają rotację ID przy loginie/zmianie uprawnień, bezwzględny i bezczynnościowy timeout, odwołanie po stronie serwera oraz odrębny zakres admina. Zmiana hasła/e-maila i usunięcie wymagają opisanej ponownej weryfikacji. Legacy change/delete muszą potwierdzać właściciela niezależnie od CAPTCHA. Sprawdzić konfigurację portów/firewall, aby nie było obejścia LAN/NetBird przez listener zaplecza. Zalecenia dotyczące rotacji sesji i timeoutów potwierdza [OWASP Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html).

**Kryterium odbioru:** testy fixation, powtórnego i równoczesnego resetu, zmiany adresu, starych sesji, CSRF oraz IDOR dla każdej operacji modyfikującej. Test XSS obejmuje profile, statusy i metadane pokazane administratorowi. Fałszywe Host/Forwarded nie zmieniają linku resetu ani tożsamości źródła limitów. Test z zewnętrznej sieci odrzuca bezpośredni port admina. Po wygaśnięciu token nie działa nawet jeśli e-mail jest jeszcze w kolejce.

### S06 — P1: niezależne limity nie stanowią jeszcze budżetu całego procesu

**Sekcja:** I07, M06/M08/M14/M16/M19/M22, §7.2, E07.

**Scenariusz ryzyka:** każde połączenie mieści się w swoim limicie, lecz ich suma wyczerpuje RAM; równoległe kosztowne logowania WWW wyczerpują pamięć/CPU; timeout zwalnia limit zadania, mimo że praca haszująca trwa dalej. Odrzucone żądania rozrastają mapę limiterów, logi, tokeny CAPTCHA lub outbox. Limit skrzynki nie ogranicza metadanych prób doręczenia ani WAL.

**Konkretna poprawka:** przed E07 dołączyć tabelę zasób → jednostka → limit globalny/per konto/per IP → TTL → właściciel → reakcja. Uwzględnić sumę buforów, liczbę zadań, współbieżne KDF/dekompresje, limity żądań HTTP, wielkość stron i koszt zapytań, subskrypcje i koszt fan-out, mapy limiterów, metadane transferów, próby doręczenia, reset/CAPTCHA/outbox/audyt, WAL i logi. Limity kosztownej pracy zakładać przed jej uruchomieniem; permit zwalniać po faktycznym zakończeniu także przy anulowaniu oczekującego requestu. Określić limit po koncie i globalny przeciw rozproszonym próbom bez łatwej trwałej blokady cudzego numeru. Bufory po przekroczeniu limitu nie mogą prowadzić do utraty już przyjętego zobowiązania.

**Kryterium odbioru:** ustalony profil obciążenia przy próbach ponad limitem utrzymuje RSS, liczbę zadań i rozmiar kolejek pod zapisanym pułapem; prawidłowy klient pozostaje obsługiwany w określonym czasie. Po timeoutach KDF liczba nadal pracujących zadań nie przewyższa limitu. Długotrwały zalew nie powiększa bez końca logów, limiterów ani bazy. Wyniki wraz z parametrami hosta trafiają do manifestu kwalifikacji.

### S07 — P1: obietnica trwałości musi wskazać klasę awarii i konfigurację magazynu

**Sekcja:** D04/D06, I01, M20, §9.1/9.3, E03/E07.

**Scenariusz ryzyka:** test `kill` procesu przechodzi, ale potwierdzona transakcja znika po awarii hosta. Plan mówi o doborze ustawień WAL do I01, lecz nie zapisuje zakresu gwarancji ani wymaganej konfiguracji. SQLite opisuje utratę trwałości WAL z `synchronous=NORMAL` przy zaniku zasilania lub awarii systemu, mimo odporności na awarię samej aplikacji. [SQLite PRAGMA synchronous](https://sqlite.org/pragma.html#pragma_synchronous).

**Konkretna poprawka:** określić osobno awarię procesu, hosta/zasilania i utratę nośnika. Dla deklaracji obejmującej pierwsze dwie ustawić i sprawdzać wymagany tryb, np. WAL plus `synchronous=FULL`, przy jawnych założeniach o lokalnym trwałym wolumenie i poprawności synchronizacji przez stos dyskowy. Zweryfikować efektywne ustawienia dla połączeń puli; nie polegać na domyślnych wartościach biblioteki. Utrata całego nośnika podlega RPO kopii, nie gwarancji samego commit. Migracje, brak miejsca i błędy I/O mają obniżać gotowość lub odmawiać nowych zobowiązań.

**Kryterium odbioru:** raport zawiera odczyt efektywnych ustawień i mapowania wolumenu kontenera, testy błędu commit/dysku i restartu procesu. Jeśli deklarowana jest odporność na awarię hosta, kwalifikacja obejmuje odpowiedni test w izolowanym środowisku lub jawnie ogranicza dostępną gwarancję; zwykły `kill` nie jest opisany jako test zaniku zasilania.

### S08 — P1: sprawny plik backupu nie wystarcza do bezpiecznego odtworzenia usługi

**Sekcja:** M20–M22, §9.3 pkt 13, §11.2 pkt 1, E07/E08.

**Scenariusz ryzyka:** kopia jest integralna, ale brakuje klucza, binarki lub konfiguracji; restore odtwarza stare sesje i wykorzystane już tokeny, albo wysyła zaległe e-maile na prawdziwe adresy. Operator ma jedynie backup z tej samej uszkodzonej maszyny. Nie ma zdefiniowanego dopuszczalnego okna utraty danych i czasu odbudowy.

**Konkretna poprawka:** opisać manifest kompletu: baza/schema, wersja i checksum artefaktu, konfiguracja bez ujawniania sekretów, wymagane `key_id` i odrębnie chronione klucze, tombstone UIN, oraz wszystkie inne wymagane dane runtime. Ustalić harmonogram, retencję, RPO/RTO, kopię poza hostem i uprawnienia do niej. Restore zaczyna się od izolacji i zapisuje nową epokę odzyskania albo jawnie odwołuje odtworzone sesje/tokeny przed otwarciem listenerów; obsługa outboxa wymaga świadomej decyzji. Opisać skutki przywrócenia historycznych haseł/blokad/usunięć po punkcie kopii i procedurę ich uzgodnienia. Nie obiecywać „usunięcia wszędzie” natychmiast, jeśli dane pozostają w retencjonowanych kopiach.

**Kryterium odbioru:** odtworzenie na pustym hoście z samego deklarowanego kompletu pozwala zalogować syntetyczne konta i odczytać oczekiwaną kolejkę; brak dowolnego wymaganego klucza daje kontrolowaną odmowę. Stare cookies/reset tokens nie odzyskują ważności, SMTP jest wyłączone podczas próby. Raport zawiera zmierzony czas, punkt danych, wyniki integralności i porównania oraz termin następnej próby.

### S09 — P1: runbook powrotu musi wybierać wykonalną ścieżkę po przyjęciu nowych danych

**Sekcja:** I12, §11.1–11.2, §12, E07/E08.

**Scenariusz ryzyka:** nowe konta, hasła, blokady, usunięcia i wiadomości powstają już w rGG, po czym występuje błąd. „Eksport zmian lub rozwiązanie zachowujące nowe dane” nie określa, jak zapisać te dane w starym schemacie ani jak uniknąć dwóch aktywnych writerów. Porównanie samych liczników nie wykryje cofniętej blokady lub przywróconej starej sesji.

**Konkretna poprawka:** dla następnych wydań rGG preferować powrót do poprzedniego zgodnego obrazu na tej samej aktualnej bazie, z tabelą zgodności binarka–schema–payload–key_id. Gdy poprzedni obraz nie czyta obecnego formatu, zatrzymać zapisy i naprawić aktualne dane/wersję do przodu. Przy pierwszym przejściu gg-retro→rGG runbook musi albo mieć sprawdzony reverse-export całego wymagającego zachowania stanu, albo jawnie wyłączać powrót do gg-retro po otwarciu ruchu i wskazywać tryb zatrzymania/naprawy. Nie wymaga to budowania reverse-importera, jeśli ścieżka jest świadomie wyłączona. Każdy wariant ma jednego aktywnego writera, kryteria uruchomienia, punkty kontrolne i weryfikację. Zachowanie nowych danych nie jest równoznaczne z natychmiastowym powrotem do starej usługi.

**Kryterium odbioru:** próba na kopii wykonuje po cutover rejestrację, zmianę hasła, zużycie zaproszenia, blokadę, usunięcie, profil i wiadomości; następnie realizuje wybraną procedurę. Żadna zmiana nie znika i żadne odwołane uprawnienie nie wraca. Uruchomienie niezgodnego obrazu zostaje odrzucone. Dla wyłączonego powrotu do gg-retro ćwiczenie potwierdza blokadę zapisów, zachowanie aktualnej bazy i wznowienie po naprawie, a nie przywrócenie starej kopii.

### S10 — P1: rejestr pochodzenia ma kontrolować rzeczywiste fragmenty, nie samą obecność wpisu

**Sekcja:** D09–D12, §8.1–8.3, §9.4, E01/E02, §15.

**Scenariusz ryzyka:** plik zawiera własne i przeniesione fragmenty, lecz ma tylko ogólny nagłówek projektu. Rejestr istnieje, ale wskazuje cały upstream, inną rewizję lub niewłaściwy plik licencji. Refaktor przenosi kod i pozostawia nieaktualny `local_paths`. Sama zmiana języka albo autora commitu nie usuwa pochodzenia portu.

**Konkretna poprawka:** zaostrzyć weryfikator o następujące reguły przypisania:

1. Każdy port/kopia/adaptacja i fixture pochodny mają trwały `id`, dokładny upstream URL, istniejącą rewizję lub identyfikator materiału, ścieżki i SHA-256 oryginału oraz źródło informacji o autorach i licencji. Zależności Cargo pozostają osobnym wykazem.
2. W pliku mieszanym przypisać wpis do symbolu lub zaznaczonego fragmentu, nie tylko całego pliku. Zachować oryginalne informacje o autorstwie; lokalny autor i zakres jego zmian są dodatkiem. SPDX nowego fragmentu nie nadpisuje automatycznie warunków pozostałych fragmentów.
3. `reference-only` wymaga krótkiego uzasadnienia, dlaczego jest to użycie opisu/idei, a nie port struktury implementacji. Zmiana języka, nazw lub układu nie stanowi takiego uzasadnienia. Wątpliwy fragment pozostaje poza włączeniem do czasu ustalenia sposobu użycia.
4. Własną licencję MIS zapisać jako jednoznaczny `LicenseRef` z pełnym niezmienionym tekstem i sumą; nie mapować na MIT. AGPL-3.0-or-later przypisywać zgodnie z nagłówkami danego pliku poggadaj. Informacja projektu libgadu o LGPL 2.1 nie rozstrzyga dokładnego wariantu SPDX dowolnego pliku.
5. CI sprawdza schemat rejestru, istnienie wskazań lokalnych i tekstów licencji, powiązania nagłówków/fragmentów oraz jawnie oznaczone braki weryfikacji. Przegląd człowieka ocenia kompletność przypisań i warunki konkretnego użycia; sam skrypt nie wykryje każdego nieoznaczonego zapożyczenia.
6. Archiwum odpowiada konkretnemu artefaktowi i zawiera potrzebne materiały budowania oraz atrybucji. Publiczny odnośnik prowadzi do tego wydania, nie jedynie do zmiennej gałęzi. Kontrola allowlisty archiwum sprawdza też przypadkowo dodane do Git dane i sekrety.

**Kryterium odbioru:** przykładowy port GG32 ma prześledzalne mapowanie symbolu Rust do przypiętego pliku Go, jego autorów i tekstu licencji. CI odrzuca brakujące źródło/hash, nieistniejące lokalne wskazanie oraz deklarację MIT dla zweryfikowanego materiału MIS. Testowy refaktor aktualizuje mapowanie. Osobny przegląd odnotowuje podstawę przypisania konkretnych fragmentów. Nie jest to gwarancja prawna zgodności całej dystrybucji.

### S11 — P2: testy wymagają stanów odmowy i mierzalnego protokołu kwalifikacji

**Sekcja:** M03–M04/M09, §9.1–9.4, §10 i §15.

**Scenariusz ryzyka:** fuzzing samych ramek przechodzi, ale sekwencja poprawnych pakietów wywołuje niedozwoloną operację przed loginem albo po blokadzie. „Pełny test bezpieczeństwa” i „test długotrwały” nie dają odtwarzalnego kryterium, więc dwa wydania zaliczają różny zakres pod tą samą nazwą.

**Konkretna poprawka:** utrzymywać macierz operacja × rola/właściciel × stan sesji × rewizja uprawnienia × transport. Obejmować rzeczywistą warstwę domenową oraz HTTP/legacy, nie tylko odpowiedź parsera. Do fuzzingu dodać sekwencje stanów, dekompresję i złożone offsety treści. Wydanie zapisuje czas/budżet fuzzingu, korpus i regresje znalezionych błędów, scenariusz długotrwały, parametry obciążenia oraz progi pamięci/czasu/błędów. Wyniki awarii i wyścigów mają jawny oracle: dozwolona powtórka legacy to inny rezultat niż utrata potwierdzonego przyjęcia. Przypiąć obrazy budowania i kroki CI do kontrolowanych wersji; build skopiowanych klientów/libgadu wykonywać bez sekretów produkcji.

**Kryterium odbioru:** manifest konkretnego SHA odsyła do kompletnej macierzy i wyników, wszystkie P1 tego raportu mają test odmowy/awarii oraz rozliczenie. Brak wyniku dla GG 6.0 nie jest zastąpiony wynikiem GG 6.1. Nowy błąd z fuzzingu trafia do trwałego korpusu regresji. Nie wymagać arbitralnej wartości procentowego pokrycia jako substytutu tych testów.

### S12 — P2: izolacja kopii i starych klientów powinna obejmować ruch wychodzący

**Sekcja:** §7.1, §9.1–9.2, §11.2 pkt 2, §12.

**Scenariusz ryzyka:** test na kopii produkcji nie przyjmuje publicznych połączeń, ale wysyła odtworzone e-maile lub korzysta z produkcyjnego discovery/relay. Stary klient testowy ładuje usługę informacyjną z zewnętrznej domeny. Kopia zawiera hasła zaszyfrowane, wiadomości, tokeny i realne adresy, mimo że środowisko nazywa się testowym.

**Konkretna poprawka:** rozszerzyć model zagrożeń o operatora, pipeline CI, archiwa, kopie i klientów testowych; przypisać zasobom wrażliwość i dopuszczalne ścieżki. W runbooku próby wymusić odizolowany adres/bind, zablokowany domyślnie egress, zastępczy transport e-mail, brak produkcyjnych danych dostępowych infrastruktury i kontrolowane DNS/discovery. Dane syntetyczne mają pierwszeństwo; kopia rzeczywistych danych ma właściciela, ograniczenia dostępu i termin usunięcia. Przechwycenia syntetycznych loginów nadal nie powinny publicznie ujawniać używanych gdziekolwiek indziej haseł.

**Kryterium odbioru:** próba odtworzenia nie nawiązuje połączeń do produkcji, SMTP ani oficjalnych usług GG; log ruchu lub test firewall potwierdza ten zakres. Raport wskazuje, które kopie zawierają realne dane, kto ma dostęp i kiedy mają zostać usunięte. Nie wykonywać takiej próby w ramach samego obecnego przeglądu.

### S13 — P2: retencja i alarmy muszą prowadzić do działania operatora

**Sekcja:** M11/M16/M22, §5, §7.2, E07/E08.

**Scenariusz ryzyka:** treści `TransportWrittenUnconfirmed`, terminalne próby i oferty obrazków pozostają latami, ponieważ retencja jest zapowiedziana, lecz nie wskazano harmonogramu sprzątania. Kopie utrwalają dane dłużej niż deklaracja produktu. Metryka wieku kolejki rośnie, ale nikt nie wie, kiedy zatrzymać przyjmowanie kolejnych zobowiązań.

**Konkretna poprawka:** dołączyć tabelę klasy danych → cel → TTL/warunek usunięcia → zadanie sprzątające → wpływ backupu. Oddzielić usunięcie treści, pozostawienie minimalnego wyniku i faktyczne odzyskiwanie miejsca. Nie deklarować bezpiecznego nadpisania nośnika na podstawie zwykłego DELETE. Dodać progi i runbooki dla wieku kolejki, miejsca/WAL, nieudanych backupów, utraty klucza i restartujących workerów. Nazwać właściciela reakcji i kanał raportowania bez nadmiarowych danych użytkownika.

**Kryterium odbioru:** test z kontrolowanym czasem usuwa treść i oferty zgodnie z regułą, zachowując jedynie uzgodnione metadane. Reguła obejmuje też `TransportWrittenUnconfirmed` i stan wstrzymania po blokadzie. Próba wywołuje alarm braku miejsca/backup failure, a runbook wskazuje bezpieczną reakcję i warunek powrotu do gotowości.

## Zweryfikowane materiały pochodzenia

- Lokalny `poggadaj/src/internal/security/gg/gg.go` w `D:\Dokumenty\Documents\ChatGPT\Serwery\gg-server-review-2026-10-30\poggadaj`, commit `4dc5484621fc53fc3205af80e4f80eca9f1d208d`: nagłówki `SPDX-License-Identifier: AGPL-3.0-or-later` i `SPDX-FileCopyrightText: 2024-2026 Oreeeee`. SHA-256 pliku: `5a5cc7d4b1164ae0477b35eb15c87f3a32ef1312c77c24021033fdf5fd4d6676`. Potwierdzenie dotyczy tego konkretnego pliku, nie automatycznie wszystkich materiałów projektu.
- Lokalny `gg-retro/LICENSE` w `D:\Dokumenty\Documents\ChatGPT\Serwery\gg-server-review-2026-10-30\gg-retro`, HEAD `85d6e498ca2e96c982d14dba82d72966b71e67d9`: tytuł MIS i autor macbury, nie standardowy tekst MIT. SHA-256: `db7bde71af91ba22f274a638c73520635c240da3e0037773a4dcc8cd49dca3e6`. To lokalny materiał referencyjny, nie potwierdzenie wszystkich nagłówków forka z commitu `ed9640c…`.
- [Oficjalna strona libgadu](https://libgadu.github.io/) wskazuje LGPL 2.1 oraz plik COPYING; dokładne nagłówki i historia konkretnego pliku pozostają do weryfikacji przed portem. Strona informuje również o zakończeniu rozwoju projektu, dlatego użycie jako klienta testowego wymaga przypięcia wersji i izolacji, a nie założenia bieżącego utrzymania.

Przegląd ustala wymagania techniczne identyfikowalności i odtwarzalności. Nie stwierdza automatycznej zgodności prawnej dowolnego połączenia fragmentów ani nie zastępuje oceny konkretnych warunków przed ich włączeniem.
