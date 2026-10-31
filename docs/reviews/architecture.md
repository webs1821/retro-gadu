# Retro Gadu — niezależny przegląd architektury i trwałości

Data: 2026-10-31. Przegląd: agent architecture. Dokument: D:\Dokumenty\Documents\RetroGadu\PLAN_IMPLEMENTACJI.md, wersja 0.1, 670 wierszy.

Zakres: granice modułów, kontrakty, sesje, transakcje, kolejka, FIFO, lease/ACK, model danych, zachowanie przy awarii oraz kolejność etapów. Odczytano cały dokument. Nie zmieniono planu ani kodu i nie łączono się z produkcją. To ocena specyfikacji, nie stwierdzenie błędów istniejącej implementacji.

## Ocena gotowości

**Plan jest dobrą podstawą projektu, ale nie jest jeszcze gotowy do bezwarunkowego zatwierdzenia architektury E02–E03.** Można rozpocząć izolowane prace fundamentu E01, takie jak workspace, parser, konfiguracja i CI. Przed utrwaleniem kontraktów sesji i kolejki trzeba rozstrzygnąć A01–A05; A06 blokuje bezpieczne zamknięcie E06. Żadna uwaga nie wymaga zmiany Rust, Tokio, SQLite ani rezygnacji z jednego procesu.

Nie znaleziono P0. P1 oznacza lukę, której błędne rozstrzygnięcie może naruszyć główne niezmienniki i wymaga jawnej decyzji przed wskazanym etapem. P2 oznacza doprecyzowanie, które nie blokuje rozpoczęcia fundamentu. Braki nie są traktowane jako dowód, że implementacja na pewno będzie błędna.

Mocne elementy planu: rozdzielenie commit od transportu i ACK, realistyczna polityka legacy bez obietnicy exactly-once, transakcyjne limity, odtwarzanie trwałej kolejki niezależnie od sygnału w pamięci, oddzielny stan LoadingInitialState, niezmienianie historii migracji i zachowanie nowych danych podczas rollbacku. Testy awarii oraz niezależne dowody protokołu są właściwymi bramkami.

## Uwagi blokujące określone etapy

### A01 — P1 — Fencing musi obejmować moment skutku, nie tylko wcześniejsze sprawdzenie sesji

**Sekcje:** I03; M09; M12; tabela offline_presence w §5; §6.1 pkt 3–4; E02. W szczególności wiersze 234–238 i 437.

**Ryzyko:** zadanie sesji S1 sprawdza session_is_current, po czym oczekuje na połączenie z bazą. W tym czasie S2 zastępuje S1. S1 wznawia się i utrwala starszy opis offline, listę kontaktów lub reguły prywatności. Sprawdzenie generacji w mapie i ochrona samego detach_session nie zapobiegają takiemu zapisowi. Podobne okno występuje pomiędzy sprawdzeniem hasła a attach_account, jeśli konto zostanie wyłączone lub zmieni credential_version.

**Dokładna zmiana:** w M09 określić punkt linearyzacji komendy i wymiany sesji. Wybrać jeden mechanizm: właściciel stanu konta serializujący komendy wraz z zakończeniem operacji DB albo atomowe warunkowe operacje magazynu z odpowiednim fencingiem. Wszystkie skutki starej sesji dotyczące konta muszą być rozliczane w tym samym porządku, także kontakt/prywatność, opis offline i rozpoczęcie doręczenia. Weryfikacja generacji tylko przed await jest niewystarczająca. Przy przejęciu konta ponownie sprawdzić aktywność konta i aktualność wersji poświadczeń użytej do uwierzytelnienia. Zdefiniować wynik komendy przyjętej przed wymianą: albo kończy się przed przejęciem, albo jest odrzucana; nie pozostawiać tego schedulerowi.

**Kryterium odbioru:** test deterministycznie zatrzymuje S1 po wstępnej autoryzacji, uruchamia S2, następnie wznawia S1. S1 nie nadpisuje stanu S2 ani nie zdejmuje jej dzierżaw. Drugi test zatrzymuje logowanie po weryfikacji hasła, zmienia credential_version/disabled_at i potwierdza odmowę użycia nieaktualnego wyniku. Zamknięcie E02 wymaga obu klas testów.

### A02 — P1 — Token dzierżawy chroni bazę, ale sam nie zatrzyma opóźnionego zapisu TCP

**Sekcje:** M06; M11, wiersze 258–262; §6.2 pkt 5–6; E03.

**Ryzyko:** worker W1 przejmuje M1 i zatrzymuje się przed wysłaniem albo po umieszczeniu ramki w kolejce writera. Lease wygasa. W2 wysyła M1, a następnie M2. W1 wznawia się i przekazuje starą M1 do tego samego albo zastępowanego socketu. Baza prawidłowo odrzuca stary token, ale bajty już wysłano. Pojedynczy writer zapobiega przeplataniu bajtów, lecz bez dodatkowego kontraktu nadal może wysłać kolejno M1, M2, M1. Również timeout zapisu krótszy od pierwotnej długości lease nie wystarcza, gdy zapis zaczyna się dopiero pod koniec lease.

**Dokładna zmiana:** określić właściciela prawa do I/O i sposób jego odwołania. Przed dopuszczeniem nowej próby nie może istnieć stary writer nadal uprawniony do rozpoczęcia zapisu tej wiadomości. Kolejka wyjściowa musi nieść co najmniej kontekst sesji/próby i termin; opóźnione pozycje nie stają się autoryzowane tylko dlatego, że wcześniej weszły do bufora. Deadline obejmuje czekanie w buforze i odnosi się do pozostałego czasu własności, nie czasu trwania lease przy przejęciu. Timeout/anulowanie po częściowym zapisie zamyka połączenie; nie można kontynuować strumienia od następnej ramki. Nie cofa się bajtów już przekazanych TCP — ta granica musi być jawna. Zdefiniować też odzyskanie prób poprzedniego procesu i zachowanie zegara; nie uzależniać odzyskania po restarcie wyłącznie od cofniętego czasu ściennego.

**Kryterium odbioru:** testy z zatrzymaniem W1 przed enqueue, w kolejce writera oraz po częściowym zapisie. Następnie wymusić utratę lease i wymianę sesji. Nowa próba zaczyna I/O dopiero po odwołaniu starego prawa do I/O; stara pozycja nie wychodzi za M2. Próby o niepewnym wyniku mogą być ponowione zgodnie z udokumentowanym kompromisem GG6. Zatrzymanie writera ma ograniczony czas i ścieżkę awarii nadzoru. Blokuje zamknięcie E03.

### A03 — P1 — FIFO koliduje z nieokreślonym wstrzymywaniem wiadomości przez blokadę

**Sekcje:** M11, wiersz 260; M13, wiersz 296; §5 messages; §6.2 pkt 4; E03–E04.

**Ryzyko:** M1 pochodzi od właśnie zablokowanego nadawcy, a M2 od innej osoby. Literalne FIFO zatrzymuje M2 za M1 na czas nieokreślony. Wybranie „najstarszej dopuszczonej” wiadomości pozwala wysłać M2, ale późniejsze wznowienie M1 narusza kolejność pierwotnego przyjęcia. M13 mówi o wstrzymaniu i nieustalonej regule odblokowania, natomiast tabela stanów nie ma stanu wstrzymania. Analogiczny problem wystąpi przy poprawnej treści niemożliwej do zakodowania dla aktualnego odbiorcy.

**Dokładna zmiana:** ustalić, czy FIFO dotyczy wszystkich przyjętych wiadomości, czy aktywnej kolejki po decyzjach kwalifikacji. Zalecany prosty wariant: nowa blokada kończy niedopuszczone jeszcze do wysłania treści stanem RejectedByPrivacy i nie odżywają one po odblokowaniu. Jeśli potrzebne jest odroczenie z powodu możliwości klienta, wprowadzić osobny stan DeferredForCapability, reguły TTL/limitów oraz jawny powrót na koniec aktywnej kolejki z nowym enqueue_order. Zachować pierwotny accepted_at dla audytu/wyświetlania. Kolejność aktywnej kolejki musi mieć deterministyczny klucz nadawany atomowo; sam zegar accepted_at ani przypadkowy UUID nie definiują kolejności przy równoczesnym przyjęciu. Wyraźnie opisać moment, od którego zmiana blokady nie cofa już rozpoczętego dispatchu.

**Kryterium odbioru:** M1 zablokowana nie zatrzymuje M2 bezterminowo; odblokowanie nie odtwarza terminalnej M1. Zmiana możliwości klienta wznawia odroczone treści według jawnej nowej kolejności, bez gorącej pętli prób i bez odnowienia TTL. Test współbieżnego przyjęcia oraz identycznych timestampów daje ten sam porządek po restarcie. Semantykę uzgodnić przed E03, implementację blokady zakończyć w E04.

### A04 — P1 — Stan wiadomości, stan próby i przejścia ACK nie są dostatecznie rozdzielone

**Sekcje:** M11, tabela stanów i polityka legacy; §5 messages/delivery_attempts; E03 i E11.

**Ryzyko:** M11 opisuje zarówno stan wiadomości, jak i „zakończenie próby”, a §5 umieszcza state wyłącznie w delivery_attempts. Nie wiadomo, jaki warunek trwale wyklucza ponowne lease zakończonej wiadomości i co wymusza tylko jedną aktywną próbę. Implementacja może zakończyć próbę legacy, usunąć ją podczas retencji, a później ponownie uznać pozostały rekord messages za Pending. Dla ACK: klient odpowiada natychmiast po zapisie TCP, przed osobnym record_transport_write. ACK może zostać odrzucone w stanie Leased albo późniejszy zapis wyniku może nadpisać już utrwalone ClientAcked.

**Dokładna zmiana:** dodać tabelę legalnych przejść z warunkami CAS i rozdzielić autorytatywny stan dostarczenia wiadomości od historii prób. Zakończenie legacy, ACK, wygaśnięcie i odrzucenie mają trwale wykluczać ponowne przejęcie, niezależnie od retencji historii prób. Baza wymusza najwyżej jednego aktualnego właściciela dla właściwego zakresu wiadomość/odbiorca; może to być jeden rekord delivery albo jawne ograniczenie aktywnej próby. Korelacja ACK z próbą musi istnieć przed I/O. Poprawne wczesne ACK nie może ginąć tylko dlatego, że osobny zapis transport_written jeszcze nie nastąpił. Stany terminalne są monotoniczne: opóźniony record_transport_write, release lub renewal nie przywraca ich do kolejki. Mapowanie pola ACK z pakietu do kontekstu próby należy do adaptera/koordynatora, nie zakładać obecności wewnętrznego lease_token w starym protokole.

**Kryterium odbioru:** dwa workery nie uzyskują równocześnie skutecznej aktywnej własności; usunięcie historii zakończonych prób nie powoduje ponownej wysyłki; ACK przed zapisem wyniku TCP i zapis TCP po ACK pozostawiają ClientAcked. Opóźnione ACK poprzedniej próby/sesji nie kończy nowej. Struktura stanu wymagana w E03; testy wire ACK obowiązują przed deklaracją profili z ACK w E11.

### A05 — P1 — Narzędzie operatora może ominąć jedynego właściciela stanu w pamięci

**Sekcje:** D03; M07; M09; M21; §3.1 rgg-admin; §6.1.

**Ryzyko:** osobny proces rgg-admin account disable zapisuje disabled_at bezpośrednio w SQLite, podczas gdy serwer utrzymuje aktywną sesję i autoryzowany kontekst konta w pamięci. Operacja zgłasza sukces, lecz klient nadal wysyła lub odbiera. Podobnie drugie uruchomienie serwera z innym portem, ale tą samą bazą, ma własny rejestr generacji. Założenie „jeden proces” nie jest samo w sobie mechanizmem wymuszającym jednego właściciela. Plan wymaga zatrzymania zapisów dla restore/migracji, ale nie rozstrzyga trybu pozostałych mutujących komend CLI.

**Dokładna zmiana:** określić tryb online i offline rgg-admin. Online mutacje kont przechodzą przez uwierzytelniony interfejs lokalny aktualnego procesu i ten sam właściciel stanu wykonuje unieważnienie sesji/lease. Offline CLI wymaga wyłącznego przejęcia katalogu danych, gdy serwer jest zatrzymany. Serwer przed otwarciem usług przejmuje i utrzymuje wyłączną blokadę katalogu danych; ta sama reguła obowiązuje migrator/importer. Wyłączność obejmuje logiczny zestaw baza/klucze/runtime, nie tylko zajęcie portu. Operacja wyłączenia/resetu/usunięcia musi jawnie rozstrzygać koniec sesji oraz skutki dla kolejki.

**Kryterium odbioru:** drugie uruchomienie na innych portach z tym samym katalogiem odmawia startu; mutujące CLI w trybie offline odmawia działania przy aktywnym właścicielu; online disable kończy uprawnienia aktywnej sesji zgodnie z określonym punktem skuteczności. Crash właściciela zwalnia blokadę w sposób umożliwiający odtworzenie. Wyłączność ustalić w E01, zachowanie mutacji kont zakończyć w E02.

### A06 — P1 — Trwałość wiadomości obejmuje również oferty obrazków i inne wymagane rekordy zależne

**Sekcje:** M10; M15; M16, wiersz 334; M20 with_transaction; §5 image_offers; §6.2 pkt 2; E06.

**Ryzyko:** accept_message zapisuje payload z odwołaniem do obrazka i potwierdza przyjęcie. record_image_offer zapisuje ofertę w osobnej transakcji. Awaria pomiędzy nimi pozostawia poprawnie wyglądającą wiadomość offline, której obrazka odbiorca nigdy nie może autoryzowanie zażądać. Sam wymóg trwałej oferty nie ustanawia jej atomowości z wiadomością. Porty aplikacji mogą też nie pozwolić składać operacji kilku repozytoriów w jedną transakcję bez ujawnienia SQL.

**Dokładna zmiana:** wyszczególnić granicę atomowego przyjęcia: payload, kontekst konferencji, obowiązkowe rekordy ofert dla zaakceptowanych adresatów, przydział kolejności i aktualizacja limitów. Zatwierdzenie całości poprzedza sukces nadawcy. Wybrać port odpowiadający całemu przypadkowi użycia lub UnitOfWork należące do aplikacji, bez sqlx w jej API. Dla konferencji określić, czy transakcja dotyczy jednego pakietu/odbiorcy czy serwerowego fan-out; częściowy sukces może istnieć wyłącznie według jawnej semantyki protokołu. Analogicznie token resetu i zlecenie jego wysłania powinny powstawać atomowo w trwałym outbox, gdy funkcja zostanie wdrożona.

**Kryterium odbioru:** wstrzyknięcie awarii między zapisami składowymi daje pełny rollback albo kompletną wiadomość z dostępnymi metadanymi; nigdy potwierdzone przyjęcie z brakującą ofertą. Odtworzenie po commit umożliwia autoryzowane żądanie obrazka, gdy nadawca jest dostępny. Kontrakt transakcji przygotować przed zamrożeniem portów E03; nie zamykać E06 bez testu obejmującego metadane obrazków.

## Sugestie i doprecyzowania nieblokujące fundamentu

### A07 — P2 — Nazwać domenę awarii gwarancji trwałości SQLite

**Sekcje:** D04/D06; I01; M20, wiersz 377; §9.1/9.3; E01–E03.

**Ryzyko:** wymaganie „commit przed ACK” i test kill procesu zostają uznane za dowód odporności również na awarię systemu/zasilania. Tymczasem dobór trybu synchronizacji ma znaczenie: w WAL ustawienie NORMAL może utracić ostatnie zatwierdzone transakcje po awarii systemu lub zasilania. To nie zarzut, że plan wymaga błędnego ustawienia — nie podaje ustawienia ani zakresu obietnicy. [Dokumentacja SQLite: synchronous](https://www.sqlite.org/pragma.html#pragma_synchronous).

**Dokładna zmiana:** zapisać, czy trwałość obejmuje awarię procesu, systemu i zasilania przy poprawnie działającym nośniku. Dla oczekiwanej trwałości commit także po awarii zasilania ustalić WAL i synchronous=FULL, wraz z weryfikacją efektywnych ustawień po otwarciu połączeń używanych do zapisu. Ewentualnie słabszy tryb oznaczyć jako osobny świadomy tryb konfiguracji, bez tej obietnicy. Opisać ograniczenie dotyczące uszkodzenia/utraty nośnika i rolę backupu, zamiast utożsamiać te zdarzenia z restartem procesu.

**Kryterium odbioru:** raport E03 oddziela kill procesu od awarii systemu/nośnika; start sprawdza oczekiwany profil trwałości. Testy oraz konfiguracja wspierają dokładnie zadeklarowany zakres, nie szerszy. Zamknąć przed kwalifikacją trwałości, bez blokowania parsera czy workspace.

### A08 — P2 — Ustalić właściciela komend i zdarzeń na granicy protocol/application

**Sekcje:** graf §3; M04 decode_command/encode_event; M14 encode_search_page; M15 encode_for_participant; E01.

**Ryzyko:** graf przewiduje protocol → domain, natomiast kontrakt decode_command zwraca „typowaną operację aplikacji”. Jeśli typ komendy mieszka w application, implementacja doda niewskazaną zależność protocol → application; próba kodowania stron katalogu lub uczestników konferencji wewnątrz application może z kolei wprowadzić zależność odwrotną. Nie jest to nieunikniony cykl, lecz nierozstrzygnięta własność kontraktu, która osłabia sens zaplanowanego check_architecture.

**Dokładna zmiana:** wybrać dozwolony graf oraz miejsce transportowo niezależnych Command/Event/DTO. Dopuszczalne jest jawne protocol → application::contracts, jeśli application nigdy nie importuje protocol, albo osobny lekki moduł kontraktów. Nie przenosić pól stricte wire do czystej domeny tylko po to, aby zachować rysunek. encode_search_page i encode_for_participant w aplikacji powinny oznaczać budowę modelu wyniku; rzeczywiste bajty i warianty wersji pozostają w adapterze.

**Kryterium odbioru:** przed rozbudową E01 jeden minimalny przepływ frame → command → use case → event → frame kompiluje się w zatwierdzonym grafie, a check_architecture sprawdza rzeczywiste zależności, w tym zakaz application → protocol/transport/SQL. Nie wymaga rozbijania małych modułów na dodatkowe crate.

### A09 — P2 — Minimalny kontrakt prywatności powinien poprzedzać testy gotowej sesji E03

**Sekcje:** M09 LoadingInitialState/Ready; M10 polityka odbiorcy; M13; tabela E02–E04 i §10.1.

**Ryzyko:** E03 ma zaliczyć pełne doręczanie, choć definicja Ready i przyjęcia odwołuje się do trwałej prywatności budowanej w E04. Bez jawnego kontraktu implementator może dodać tymczasowy bypass Ready lub allow-all, a potem pozostawić go w ścieżce publicznej. Nie jest to sprzeczność uniemożliwiająca pracę: izolowany test E03 może korzystać z kontrolowanej polityki testowej.

**Dokładna zmiana:** do E02/E03 przenieść minimum: interfejs polityki, bezpieczny trwały stan domyślny, wersję zasad i bramkę Ready. E04 nadal implementuje pełny import kontaktów, subskrypcje i warianty widoczności. Dopuszczony test double prywatności ma być wyłącznie fixture, nie konfigurowalnym publicznym trybem omijania reguł. A03 określa wcześniej wspólną semantykę kolejki i blokady.

**Kryterium odbioru:** scenariusz E03 przechodzi przez ten sam kontrakt Ready co późniejszy E04. Przed osiągnięciem Ready żadna wiadomość ani obecność nie opuszcza serwera, także przy opóźnionym odczycie zasad i błędzie odtwarzania. E04 rozszerza reguły bez przebudowy cyklu sesji.

## Ocena propozycji rozstrzygnięcia przesłanej podczas przeglądu

Autor rozważa: jeden proces z wyłączną blokadą katalogu danych; właściciela stanu per konto serializującego komendy wraz z operacjami DB; osobnego writera per sesja; zakończenie starego writera przed Ready nowej sesji; zakaz nowego I/O po samym wygaśnięciu lease, dopóki nie wycofano starego prawa do I/O; terminalne RejectedByPrivacy; DeferredForCapability poza aktywnym FIFO.

**To spójny kierunek rozwiązania A01–A05, pod następującymi warunkami:**

- **Brak cyklicznego oczekiwania między właścicielami kont.** Jeżeli actor A obsługując wiadomość czeka na actor B, a B w tym czasie obsługuje odpowiedź i czeka na A, nastąpi deadlock. Kontrakt musi wskazać właściciela operacji wielokontowej i przekazywanie komend bez cyklu oczekiwań. Nie trzymać blokującej obsługi aktora nad wywołaniem innego aktora, który może wymagać odpowiedzi pierwszego. Dotyczy również konferencji i transferów. Test: równoczesne A→B i B→A oraz aktualizacja blokady kończą się w zadanym czasie.
- **Bariera writera ma deadline i ograniczony zakres.** Nowa sesja może czekać na zakończenie starego I/O tylko przez ograniczony czas. Po niepewnym lub częściowym zapisie zamyka się socket; zawieszone zadanie uruchamia ustaloną ścieżkę nadzoru. Nie utrzymywać globalnego mutexa ani transakcji SQL podczas tego oczekiwania. Dla nowej sesji brak jej Ready nie może blokować innych kont.
- **Cofnięcie prawa do I/O obejmuje bufor writera.** Zakończenie producenta nie wystarcza, jeżeli writer nadal ma jego ramki. Bariera ma potwierdzać, że nie rozpocznie już ich zapisu. Nie gwarantuje wycofania bajtów już przekazanych transportowi; ten fakt zgadza się z ograniczeniami legacy.
- **RejectedByPrivacy ma określony moment skuteczności.** Nie można obiecywać cofnięcia rozpoczętego dispatchu. Po zatwierdzeniu blokady nowe dispatch nie rozpoczynają się według starej rewizji zasad. Kolejka wyjściowa musi respektować tę granicę, a terminalizacja w bazie nie może udawać, że wcześniej wysłane bajty nie istniały.
- **DeferredForCapability wymaga nowego porządku przy wznowieniu.** Przyjęcie starej wiadomości ponownie na ogon aktywnej kolejki rozwiązuje blokowanie kolejnych treści, lecz jest świadomym osłabieniem FIFO pierwotnego przyjęcia. Zachować accepted_at i osobny enqueue_order; nie zerować TTL, nie usuwać z naliczania limitów przechowywania i nie retryować bez zmiany możliwości. Odroczenie dotyczy poprawnej treści nieobsługiwanej przez klienta; uszkodzony lub nieznany dla serwera format payload wymaga osobnego błędu, nie cichego wiecznego odroczenia.
- **Widoczność w WWW zwiększa zakres samoobsługi.** M19 dziś nie obiecuje widoku odroczonych własnych wiadomości. Jeśli będzie on sposobem poinformowania użytkownika, dodać go jawnie do E05 i testów uprawnień, bez udostępniania administratorowi treści. W przeciwnym razie użytkownik starego klienta pozostaje bez wiedzy o odroczeniu.
- **Blokada katalogu współdziała z CLI i restartem.** Sam lock nie rozwiązuje online account disable. Zastosować model A05. Po wyłącznym odzyskaniu katalogu można zidentyfikować próby poprzedniej epoki procesu jako pozbawione żywego writera i odzyskać je bez czekania na odległy czas ścienny.

Te propozycje nie zostały potraktowane jako już obecne w ocenianej wersji 0.1. Przegląd powinien zostać rozliczony tabelą decyzji i testów po aktualizacji planu, zgodnie z §13.

Autor następnie doprecyzował, że ograniczony coordinator przekazuje komendy i zbiera wyniki poza pętlami kont, actor nie oczekuje na wynik innego actora, a wymagane credential_version/status/revision/fencing są sprawdzane przez SQLite atomowo z zapisem. FIFO ma obejmować aktywną kolejkę; wznowiona DeferredForCapability dostaje nowy enqueue_order przy zachowaniu accepted_at. **Te doprecyzowania rozwiązują wskazane kontrprzykłady cyklicznego oczekiwania, sprawdzenia przed await oraz niejawnej kolejności wznowienia.**

Pozostają dwa warunki implementacyjne do zapisania przy tym rozwiązaniu. Coordinator musi mieć limit liczby operacji, deadline oraz regułę anulowania i wyników częściowych. Ponadto oczekiwanie actora na koniec writera nie może tworzyć drugiego cyklu: actor czeka na join, podczas gdy writer czeka na obsłużenie przez ten sam actor raportu transport_written. Potwierdzenie utraty prawa do I/O musi być niezależne od opróżnienia callbacków, a opóźnione raporty przechodzą sprawdzenie tokenu i monotoniczne przejścia stanu z A04. Test zatrzymania sesji powinien celowo wymusić taki przeplot.

## Minimalna bramka akceptacji architektury

1. E01: dozwolony graf zależności oraz egzekwowana wyłączność katalogu danych.
2. E02: linearyzacja komend, zmiany generacji i administracyjnego unieważnienia uprawnień.
3. E03: autorytatywny model stanu dostarczenia, atomowe przejścia, granica prawa do I/O, jawne FIFO i przypadki odroczenia/odrzucenia.
4. E06: transakcja wiadomości wraz z ofertami obrazków i ustaloną semantyką konferencji.
5. Kwalifikacja: scenariusze z kontrolowanym przeplotem z A01–A06, a nie tylko nieukierunkowany test obciążenia.

Po tych decyzjach kolejność E01–E12 jest zasadniczo wykonalna. Nie ma potrzeby projektowania klastra ani wielu backendów z wyprzedzeniem; opisane problemy dotyczą także jednego procesu i powinny być rozwiązane w jego prostym modelu własności.
