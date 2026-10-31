# Dodatek do przeglądu bezpieczeństwa: plan Retro Gadu v0.2

Data: 2026-10-31.

Sprawdzono `D:\Dokumenty\Documents\RetroGadu\PLAN_IMPLEMENTACJI.md`, wersję 0.2, SHA-256 `54ba707b2958f3dc3fe85984e3a4f1157bbfc33fe8b8a714c2c5ea4c241298b2`, oraz `docs/reviews/ROZLICZENIE_UWAG.md`. Zakres ograniczono do wcześniejszych S01–S13. Nie zmieniono planu ani pierwszego raportu, nie sprawdzano działającej implementacji i nie łączono się z produkcją.

**Ocena: można rozpocząć E01.** Wszystkie S01–S13 zostały uwzględnione specyfikacyjnie w stopniu wystarczającym dla planu. Nie pozostał w tym zakresie P0 ani P1 blokujący rozpoczęcie fundamentu. To potwierdzenie gotowości planu do lokalnej implementacji, nie zaliczenie testów ani dopuszczenie wdrożenia.

## Kontrola rozliczenia

| ID | Wynik | Podstawa w poprawionym planie |
| --- | --- | --- |
| S01 | Uwzględnione specyfikacyjnie | §3.2–3.4 i M08–M09 określają atomowe porównanie credential_version, aktywności i generacji przy dołączeniu oraz skutku komendy. Reset/blokada unieważniają sesje/tokeny; profile otrzymują listę operacji dozwolonych w stanie. M21 obejmuje również CLI wspólną ścieżką albo wyłącznością offline. |
| S02 | Uwzględnione specyfikacyjnie | §3.3 wprowadza zamknięcie dispatch, barierę nierozpoczętych ramek, commit i publikację rewizji. Rozdziela niewysłaną kolejkę od bajtów oddanych TCP, określa timeout i zamknięcie niepełnej ramki oraz unika oczekiwania na callback przez zablokowanego actora. |
| S03 | Uwzględnione specyfikacyjnie | M07, reserved_uins w §5 i §11.1 zakazują ręcznego i automatycznego odziedziczenia usuniętego numeru. Tombstone przechodzi przez backup/import; brak historycznych danych źródła ma jawny raport i rozstrzygnięcie operatora. |
| S04 | Uwzględnione specyfikacyjnie | M08 wybiera wersjonowaną kopertę AEAD, nonce/CSPRNG/AAD, kontrolę pary key/nonce, odmowę przy błędzie i wznawialny keyring. §11.3 obejmuje pełny komplet kluczy oraz nowy klucz zapisu po restore. Dobór konkretnej biblioteki i jej audyt są bramką E02. |
| S05 | Uwzględnione specyfikacyjnie | M18–M19 wymagają odrębnego dowodu tożsamości legacy, zweryfikowanego kanału resetu, atomowego zużycia tokenu, kontrolowanego originu i ochrony linku/outboxa. Są rotacja i odwołanie sesji, reautoryzacja operacji wrażliwych oraz kontrola obejścia listenera admina. Tabela rozliczenia zachowuje testy fixation, CSRF/IDOR/XSS, Host i starych tokenów. |
| S06 | Uwzględnione specyfikacyjnie | §7.2 ma rejestr zasobów, globalną pulę obok limitów lokalnych, ograniczenie KDF/dekompresji przed startem i zwolnienie permitu dopiero po zakończeniu pracy. Obejmuje mapy, metadane, outbox, WAL i dysk. R0 jest jawnie celem pomiarowym, a wartości liczbowe i wyniki pozostają wymogiem E07. |
| S07 | Uwzględnione specyfikacyjnie | M20 wybiera WAL/FULL i weryfikację ustawień połączeń zapisujących, określa trwały lokalny wolumen oraz rozdziela awarię procesu, hosta i nośnika. Raport testu musi wskazywać faktycznie sprawdzoną klasę awarii; kill procesu nie zastępuje testu zaniku zasilania. |
| S08 | Uwzględnione specyfikacyjnie | §11.3 definiuje komplet odtworzeniowy, kopię poza hostem, cele RPO/RTO, izolację, odwołanie starych sesji/tokenów, wstrzymanie outboxa oraz uzgodnienie historycznych zmian uprawnień. Próba ma działać na pustej instancji z samego wskazanego kompletu. |
| S09 | Uwzględnione specyfikacyjnie | §11.2 wybiera zgodny poprzedni obraz rGG na aktualnej bazie albo zatrzymanie zapisów i naprawę do przodu. Powrót do gg-retro po nowych zapisach jest wyłączony bez sprawdzonego reverse-exportu. Są pojedynczy writer i próba zachowania zmian po cutover. Nie wymaga to fikcyjnej gwarancji natychmiastowego powrotu do starej aplikacji. |
| S10 | Uwzględnione specyfikacyjnie | §8.2–8.3 rozszerza rejestr o symbole/fragmenty i dowody przypisania, zachowuje LicenseRef-MIS-macbury i pełny tekst, rozróżnia port od reference-only, wymaga aktualizacji po refaktorze i kontroli źródeł konkretnego wydania. Przegląd człowieka uzupełnia skrypt; nie pada gwarancja prawna. |
| S11 | Uwzględnione specyfikacyjnie | §9.3–9.4 dodaje deterministyczne przeploty, macierz stanów/uprawnień/transportów, fuzzing sekwencji i dekompresji oraz zapis budżetu, korpusu, progów i dokładnego artefaktu. Manifest ma odróżniać dozwoloną powtórkę legacy od utraty zobowiązania. |
| S12 | Uwzględnione specyfikacyjnie | §11.4 wymaga blokady egress, atrapy poczty, kontroli DNS/discovery, braku sekretów infrastruktury u starych klientów i narzędzi oraz właściciela, dostępu i terminu usunięcia kopii rzeczywistych danych. |
| S13 | Uwzględnione specyfikacyjnie | §7.3 zawiera tabelę TTL/sprzątania treści, prób, ofert, tokenów, stagingu, logów, tombstone i backupów. Alarmy mają progi, właściciela oraz runbook. Usunięcie aktywnych danych nie jest mylone z ich natychmiastowym usunięciem ze wszystkich kopii ani nadpisaniem nośnika. |

## Co pozostaje do wykonania

Nie wskazuję nowej konkretnej luki wymagającej kolejnej zmiany architektury przed E01. Tabela rozliczenia poprawnie traktuje zamknięcie uwag jako zmianę specyfikacji, nie jako dowód wykonania zabezpieczeń.

Nadal obowiązują bramki już zapisane w planie: działający przepływ i blokada katalogu w E01; biblioteka kryptograficzna, konfiguracja kluczy i kontrolowane przeploty w E02; transakcje, writer, retencja i testy awarii w E03–E04; testy WWW i odmów w E05; liczbowe limity z pomiarów oraz faktyczny restore w E07; przećwiczony runbook zachowujący nowe dane przed E08. Każde zapożyczenie wymaga rzeczywistego wpisu i sprawdzenia konkretnego materiału przed włączeniem.

Weryfikacja tych punktów ma nastąpić przy realizacji właściwych etapów. Brak istniejącego kodu i raportów pomiarowych na etapie planu nie jest dodatkowym blokerem rozpoczęcia E01.
