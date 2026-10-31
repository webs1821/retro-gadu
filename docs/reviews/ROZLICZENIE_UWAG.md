# Retro Gadu — rozliczenie niezależnych przeglądów planu

Data: 2026-10-31. Plan wejściowy: v0.1, SHA-256 `380d006a7244b65d1e9f4c38365c0f99f3b75acb5b5e6f7cbe54b74b2826d67b`.

Trzech agentów przeczytało plan niezależnie: architektura i trwałość, protokół oraz bezpieczeństwo/testy/operacje. Pierwsza runda: **28 uwag, 19 P1 i 9 P2, brak P0**. Druga runda dodała uściślenie A04-R1 (P2), również naniesione. P1 jest warunkiem wskazanej funkcji/etapu; nie oznacza potwierdzonej podatności działającego serwera.

Raporty: [architektura](architecture.md), [protokół](protocol.md), [bezpieczeństwo](security.md). Oceniany szkic zachowano jako [PLAN-v0.1.md](PLAN-v0.1.md). Aktualny dokument: [PLAN_IMPLEMENTACJI.md](../../PLAN_IMPLEMENTACJI.md).

„Uwzględniono” poniżej oznacza zmianę specyfikacji i dodanie kryterium odbioru. Nie oznacza napisania kodu, wykonania testu ani potwierdzenia pełnej zgodności protokołu. Agentom przekazano [wersję 0.2](PLAN-v0.2.md) do ponownej kontroli tych zmian; wszystkie trzy kontrole zakończyły się pozytywną oceną gotowości rozpoczęcia E01.

## Architektura

| ID | Waga | Rozstrzygnięcie i miejsce w planie | Bramka implementacyjna |
| --- | --- | --- | --- |
| A01 | P1 | §3.2, M08–M09: actor konta, atomowe wersje/generacja i warunkowe mutacje przy skutku; brak samego check-before-await. | E02: wymiana sesji/reset zatrzymany w kontrolowanym przeplocie. |
| A02 | P1 | §3.3–3.4, M11: prawo do I/O, bariera starego writera, deadline obejmujący kolejkę, zamknięcie po częściowym zapisie, epoka procesu. | E03: opóźniony stary writer nie wysyła za nową próbą; brak deadlocku callbacków. |
| A03 | P1 | M11/M13: FIFO aktywnej kolejki, atomowy enqueue_order, terminalne RejectedByPrivacy, odroczenie możliwości i powrót na koniec bez zmiany TTL. | E03/E04, a odroczenie przed mieszanymi profilami E09/E11. |
| A04 | P1 | M11/§5: osobne deliveries i historia prób, legalne przejścia CAS, wczesne ACK i monotoniczne stany końcowe. | E03 i wire ACK przed E11. |
| A05 | P1 | D03, §3.4, M21: wyłączny lock katalogu; CLI online przez serwer, offline z wyłącznością. | E01/E02: drugi proces i omijające stan CLI odmawiają. |
| A06 | P1 | §3.4/M10/M16: accept_message_bundle obejmuje treść, kontekst, oferty, kolejność i limity w jednej transakcji. Reset/outbox również atomowe. | Kontrakt E03, testy całego bundle E06. |
| A07 | P2 | M20/§11.3: WAL/FULL, rzeczywiste ustawienia, klasy awarii oraz RPO dla utraty nośnika. | E03/E07: nie utożsamiać kill procesu z testem zaniku zasilania. |
| A08 | P2 | §3: Command/Event w application::contracts, protocol zależy od kontraktów, aplikacja nie od protokołu. Kodowanie bajtów tylko w adapterze. | E01: kompilowany przepływ i kontrola grafu zależności. |
| A09 | P2 | M09/E02/E03: minimalna trwała prywatność i Ready przed uruchomieniem dostarczania; pełne kontakty rozwijane w E04. | Brak trybu produkcyjnego allow-all i wysyłki przed Ready. |

## Protokół

| ID | Waga | Rozstrzygnięcie i miejsce w planie | Bramka implementacyjna |
| --- | --- | --- | --- |
| P01 | P1 | M13/E04: staging i poprzednia wersja; brak obietnicy pełnej kompletności z ciszy/FIN. Osobna decyzja contact_upload_completion per profil. | **Badanie pozostaje otwarte**: długości 0/1/2046/2047/2048/4094/4095, ACK, przerwania i natywny klient przed ogłoszeniem obsługi uploadu. |
| P02 | P1 | M18/E05: pierwsza rejestracja wyłącznie WWW z zaproszeniem; natywny formularz odmawia bez obejścia i bez korelacji po IP. | Utworzenie WWW → login GG; natywna próba nie tworzy konta ani nie zużywa kodu. |
| P03 | P1 | M11: osobna korelacja wire_seq do sesji/generacji/próby/tokenu przed I/O, brak założenia globalnej unikalności seq nadawcy. | E11: potwierdzenie punktowego/kumulatywnego ACK i numeracji retransmisji, wrap, stare ACK i wczesne ACK. |
| P04 | P2 | M02/M10: MessageSemantics, maski msgclass, brak żądanego ACK, oddzielenie trwałej wiadomości od sterowania transferem. | E03/E06: klasy, puste komendy obrazków i UI odpowiedzi. |
| P05 | P2 | M11/M19/E09: DeferredForCapability poza aktywnym FIFO; nowy porządek przy wznowieniu, niezmienny expiry, limity i informacja w samoobsłudze. | Przed mieszanymi profilami: nowy → stary → nowy klient bez utraty i gorącej pętli. |
| P06 | P2 | §9.2: historyczny/przypięty klient libgadu rzeczywiście wysyłający GG_LOGIN60; asercja pakietu, nie samego parametru biblioteki. | E01/E02: manifest narzędzia i rzeczywisty login; natywne buildy osobno. |

Uwzględniono również doprecyzowania raportu: SHA1/LOGIN70 w odpowiednim GG7.x, kody błędów katalogu, dokładna liczba kopii konferencji, życie oferty obrazka po usunięciu treści oraz jawne wyłączenie niegotowego DCC z deklaracji pierwszego wydania GG6.

## Bezpieczeństwo, testy i operacje

| ID | Waga | Rozstrzygnięcie i miejsce w planie | Bramka implementacyjna |
| --- | --- | --- | --- |
| S01 | P1 | §3.2/M08/M09: credential_version/generacja przy dołączeniu i skutku; atomowa zmiana sekretów i odwołanie tokenów/sesji. | E02/E05: reset/blokada w trakcie loginu, podwójny reset, operacje w złym stanie. |
| S02 | P1 | §3.3/M11/M13: bramka dispatch i bariera, nowa rewizja zasad, unieważnienie nierozpoczętych ramek; jawna granica bajtów już oddanych TCP. | E04: wolny writer, wiadomości/statusy/obrazy i zmiana prywatności. |
| S03 | P1 | M07/§5/§11.1: trwała rezerwacja usuniętego numeru bez danych osobowych; raport braków historycznych źródła importu. | E05: brak odziedziczenia starej tożsamości po delete/restart/import. |
| S04 | P1 | M08/§11.3: wybrana koperta AEAD, nonce/CSPRNG/AAD, keyring, wznawialna rotacja, zestaw key_id w backupie, nowy klucz zapisu po restore. | E02/E07: błąd losowości, zamiana ciphertext, uszkodzenie, przerwanie rotacji i brak klucza. |
| S05 | P1 | M18/M19: zweryfikowany kanał resetu, atomowe tokeny, sekret outbox, link/origin/referrer, rotacja i odwołanie sesji, niezależny admin. | E05: fixation, CSRF/IDOR/XSS, Host, stary token i obejście portu admina. |
| S06 | P1 | §7.2: globalny budżet obok per-konto/IP, KDF/dekompresja, realne zwalnianie permitu, mapy/metadata/WAL; profil pomiarowy R0. | E07: konkretne limity i pomiary sumarycznego RSS/zadań/opóźnień. |
| S07 | P1 | M20/§11.3: WAL/FULL, efektywne PRAGMA, trwały lokalny wolumen i rozdzielenie klas awarii. | E03/E07: kwalifikacja dokładnie deklarowanej gwarancji. |
| S08 | P1 | §11.3: pełny manifest, oddzielne klucze, kopia poza hostem, cel RPO/RTO, odwołanie odtworzonych tokenów i wstrzymany outbox. | E07/E08: odtworzenie na pustej instancji z samego deklarowanego kompletu. |
| S09 | P1 | §11.2: kompatybilny rollback rGG na aktualnej bazie lub forward-fix. Powrót do gg-retro po ruchu domyślnie wyłączony bez sprawdzonego reverse-export. | E08: próba po rzeczywistych zmianach testowych i jeden aktywny writer. |
| S10 | P1 | §8.2–8.3: mapowanie symboli/fragmentów, LicenseRef-MIS, dowody licencji/autorów, kontrola hasha i refaktorów, przegląd poza automatem. | Każde zapożyczenie i wydanie; brak deklaracji gwarancji prawnej. |
| S11 | P2 | §9.3–9.4: kontrolowane przeploty, macierz stanów/uprawnień/transportów, fuzzing sekwencji i jawny budżet kwalifikacji. | E01/E07: manifest z konkretnymi wynikami, nie samą nazwą testu. |
| S12 | P2 | §11.4: blokada egress, atrapa SMTP, kontrola DNS, brak sekretów produkcyjnych w narzędziach, właściciel i TTL kopii rzeczywistej. | Przed użyciem kopii danych i testami starych klientów. |
| S13 | P2 | §7.3: tabela retencji, sprzątanie treści/stanów/ofert i oddzielna retencja kopii; alarm z runbookiem i właścicielem. | E03/E07: kontrolowany czas i wywołanie alarmów w próbie. |

## Stan końcowej kontroli

Wersja 0.2 otrzymała trzy dodatki: [architektura](architecture-followup.md), [protokół](protocol-followup.md), [bezpieczeństwo](security-followup.md). Oryginalne raporty zachowano bez nadpisywania. Każdy agent potwierdził gotowość planu do rozpoczęcia E01.

Agent architektury wskazał A04-R1: brak jawnego ponowienia ze stanu AwaitingClientAck. W finalnym M11 dopisano przejście do Pending po timeout/restart, z barierą starego I/O, CAS, zamknięciem starej korelacji, zachowaniem enqueue_order/expiry oraz nowym tokenem i numeracją właściwą dla profilu. ClientAcked wyklucza retry. Treść rozstrzygnięcia zaakceptowano w dodatku; v1.0 zawiera ten wiersz oraz aktualizację statusu dokumentu względem ocenionej v0.2. Nie przedstawiamy tego jako ponownego niezależnego odczytania całej v1.0 przez agentów.

**Wynik końcowy: plan v1.0 jest gotowy do rozpoczęcia lokalnego fundamentu E01.** Wszystkie uwagi mają decyzję, miejsce w specyfikacji i bramkę weryfikacji implementacyjnej.

Przed implementacją konkretnego etapu nadal trzeba wykonać jego badania/testy: w szczególności granicę uploadu kontaktów, dostępne buildy natywne, semantykę ACK późniejszych profili i pomiary limitów. Są to nazwane zadania z bramkami, a nie potwierdzone już możliwości rGG.
