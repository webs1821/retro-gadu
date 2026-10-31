# Przegląd zgodności protokołu Retro Gadu

Data: 2026-10-31. Zakres: PLAN_IMPLEMENTACJI.md v0.1 z D:\Dokumenty\Documents\RetroGadu. Przegląd statyczny dokumentacji i kodu; nie wykonywano połączeń z produkcją ani testów natywnych. Nie zmieniano planu ani implementacji.

Ocena: podstawowy kierunek jest poprawny. Nie wykryto problemu P0. Dwie decyzje wymagają rozstrzygnięcia przed pierwszym wydaniem GG6: granica atomowego uploadu kontaktów i sposób rejestracji z zaproszeniem. Dalsze uwagi dotyczą konkretnych kontraktów i bramek późniejszych etapów, nie są twierdzeniem, że przyszły serwer już ma te błędy.

Źródła lokalne: gg-retro HEAD `ed9640cc247764754b5e47891053eb07a53f76d5`; poggadaj HEAD `4dc5484621fc53fc3205af80e4f80eca9f1d208d`. Odczytane źródła libgadu z `master` są materiałem do analizy na dzień przeglądu; przed wykorzystaniem jako fixture należy przypiąć konkretny commit.

## P01 — P1 — Atomowość uploadu kontaktów nie ma zdefiniowanego końca operacji

**Sekcja:** M13, linia 294; §9.3 punkt 8; E04.

**Fakt i dowód:** stary upload ma PUT/PUT_MORE bez osobnego typu końcowego. `gg_userlist_request()` w [libgadu.c](https://raw.githubusercontent.com/wojtekka/libgadu/master/src/libgadu.c) wysyła fragmenty do 2047 bajtów; ostatni fragment może mieć dokładnie 2047 bajtów, bez dodatkowego pustego terminatora. Lokalny [fork, session.rs:215](D:/Dokumenty/Documents/gg-retro/server/src/messenger/session.rs:215) zapisuje od razu pierwszy fragment i dopisuje kolejne. Z kolei [poggadaj, handlers.go:162](D:/Dokumenty/Documents/ChatGPT/Serwery/gg-server-review-2026-10-30/poggadaj/src/cmd/poggadaj-tcp/handlers.go:162) czeka na fragment krótszy niż pełny. Te implementacje nie dostarczają bezpośrednio wymaganej gwarancji planu.

**Scenariusz i wniosek:** po pełnym bloku nie wiadomo, czy otrzymano całą listę długości 2047, czy początek dłuższej listy. Zatwierdzenie po ciszy może opublikować urwany import; oczekiwanie wyłącznie na krótszy blok może nigdy nie zakończyć poprawnego transferu. To wniosek z dostępnego formatu, a nie rezultat nowego testu GG6. Natywny klient może używać dodatkowej konwencji, którą trzeba dopiero wykazać.

**Dokładna poprawka:** dodać do E04 blokującą decyzję `contact_upload_completion(profile)`, określającą dowód końca transferu i moment odpowiedzi dla każdego fragmentu. Staging ma zachowywać dotychczasową wersję; timeout ani FIN nie mogą być automatycznie uznane za dowód pełnego uploadu. Jeśli dokładny klient nie daje rozstrzygającego znacznika, opisać ograniczenie protokołu i zatwierdzoną politykę wersjonowania/odmowy niejednoznacznego uploadu, zamiast bezwarunkowej gwarancji atomowości całej listy. Nie kopiować bez sprawdzenia żadnej z dwóch heurystyk.

**Test:** natywne GG6.0 i 6.1 oraz niezależny klient: payload 0, 1, 2046, 2047, 2048, 4094 i 4095 bajtów; przerwanie po każdym pełnym bloku; opóźniony kolejny blok; ponowny PUT w tej samej sesji; eksport w trakcie stagingu i po restarcie. Fixture musi uwzględniać kolejność ACK oraz ewentualny terminator, nie tylko połączony tekst.

## P02 — P1 — Rejestracja natywna nie ma drogi przekazania zaproszenia

**Sekcja:** M07, M18, E05 (kryterium „rejestracja i katalog w prawdziwym GG6”).

**Fakt i dowód:** opis natywnej rejestracji wymienia `pwd`, `email`, `tokenid`, `tokenval`, `code`; nie ma pola zaproszenia. [Dokumentacja libgadu, §2.3](https://libgadu.github.io/protocol/). Lokalny [register.rs:10](D:/Dokumenty/Documents/gg-retro/server/src/api/register.rs:10) wymaga dodatkowego `invite` i odmawia przy jego braku. Samo użycie endpointu o starej nazwie nie zapewnia zgodności z formularzem GG.

**Scenariusz:** użytkownik wypełnia standardowy formularz GG6 poprawnymi danymi, lecz serwer zawsze odrzuca rejestrację; usunięcie wymagania zaproszenia z tej ścieżki naruszyłoby M18.

**Dokładna poprawka:** autor planu wskazał podczas przeglądu rozwiązanie do wpisania do dokumentu: pierwsze wydanie rejestruje konta wyłącznie przez WWW z zaproszeniem. Formularz natywny otrzymuje udokumentowaną odpowiedź braku dostępności oraz link tylko tam, gdzie klient go potrafi przedstawić. E05 sprawdza natywny katalog i logowanie kontem utworzonym przez WWW. Zmiana hasła/usuwanie konta przez legacy HTTP wymagają osobnego, uwierzytelnionego kontraktu. Nie wiązać zaproszeń po samym IP.

**Status:** rozstrzygnięcie przekazane przez autora w toku przeglądu, ale jeszcze nie obecne w ocenianym v0.1; po wpisaniu do planu uwaga może zostać zamknięta.

**Test:** utworzenie konta WWW z ostatnim użyciem zaproszenia i logowanie GG6; natywna próba rejestracji nie tworzy konta, nie zużywa zaproszenia i nie zawiesza formularza.

## P03 — P1 przed E11 — ACK klienta nie zawiera tokenu dzierżawy ani MessageId

**Sekcja:** M04, M11, model `delivery_attempts`, E11.

**Fakt i dowód:** [libgadu protocol.h, `gg_recv_msg_ack`](https://raw.githubusercontent.com/wojtekka/libgadu/master/include/protocol.h) zawiera wyłącznie 32-bitowy `seq`. Plan wymaga natomiast aktualnego MessageId, tokenu dzierżawy i generacji. Nie opisuje warstwy korelacji tych dwóch modeli. Już obecny [fork, messages.rs:119](D:/Dokumenty/Documents/gg-retro/server/src/messenger/messages.rs:119) przekazuje odbiorcy numer sekwencji nadawcy; nie wolno przyjąć, że ta strategia automatycznie działa po dodaniu odbiorczych ACK.

**Scenariusz:** dwóch nadawców używa tego samego `seq`; albo wygasa próba, wiadomość dostaje nową dzierżawę i przychodzi spóźnione ACK poprzedniej próby. Wyszukanie po samym numerze może zakończyć niewłaściwą wiadomość lub nową dzierżawę. Sama kontrola tokenu w bazie nie pomoże, jeśli adapter dobrał do spóźnionego ACK aktualny token.

**Dokładna poprawka:** opisać osobny numer transmisji do odbiorcy i mapowanie `(SessionId, generation, wire_seq) -> (MessageId, attempt, lease_token)`. Numer nadawcy pozostaje korelacją odpowiedzi dla nadawcy. Mapowanie odbiorcze powstaje przed możliwym nadejściem ACK, znika po zakończeniu próby i nie może zostać ponownie wykorzystane tak, aby stary pakiet potwierdził nowe zobowiązanie. Zdefiniować wrap-around, timeout/retry i zachowanie duplikatów. Przydzielanie nowego numeru przy retransmisji wymaga potwierdzenia przez test konkretnego klienta; nie przedstawiać go jako już zweryfikowanego. Oddzielnie rozstrzygnąć, czy ACK danego profilu jest punktowe czy kumulatywne.

**Test:** identyczne `seq` różnych nadawców, powtórka tego samego `seq` z inną treścią, duplikat ACK, ACK nieznany i opóźniony po wygaśnięciu dzierżawy, zastąpienie sesji i zawinięcie licznika. Natywny test z włączoną i wyłączoną możliwością ACK. Nie blokuje to samodzielnego wydania GG6 bez tej funkcji.

## P04 — P2 — Kontrakt wiadomości pomija semantykę klasy i wyjątków od ACK/kolejkowania

**Sekcja:** M02 `MessageEnvelope`/`MessageContent`, M10, M16, E03/E06.

**Fakt i dowód:** [`libgadu.h.in`](https://raw.githubusercontent.com/wojtekka/libgadu/master/include/libgadu.h.in) definiuje klasy jako maski: `GG_CLASS_ACK` oznacza brak żądania potwierdzenia, `GG_CLASS_QUEUED` jest cechą odbiorczą, CTCP jest komunikatem klienta. Lokalny [consts.rs:107](D:/Dokumenty/Documents/gg-retro/protocol/src/consts.rs:107) zachowuje kombinacje bitów. [session.rs:450](D:/Dokumenty/Documents/gg-retro/server/src/messenger/session.rs:450) osobno respektuje brak ACK dla pakietów obrazka. W planie kolejność M10 jest opisana bezwarunkowo, a domenowy model wiadomości nie wymienia semantyki klasy.

**Scenariusz:** implementacja jednego wspólnego `accept_message` może potwierdzać także pakiety z wyłączonym ACK, potraktować pustą treść żądania/fragmentu obrazka jako zwykłą wiadomość albo włożyć ulotne sterowanie transferem do trwałej kolejki. Nie jest to stwierdzenie błędu przyszłego kodu; to brak decyzji potrzebnej przy implementowaniu tego kontraktu.

**Dokładna poprawka:** rozdzielić typowaną wiadomość użytkownika i polecenie transferu przed wejściem do M10; określić, które operacje są trwałe. Zachować semantykę `msgclass` w metadanych adaptera/domenowych, w tym politykę odpowiedzi nadawcy. Przyjęcie trwałej wiadomości nadal wymaga commit, także gdy klient nie żąda odpowiedzi. Dopisać tabelę wyniku domenowego -> dozwolony wire ACK per profil, wraz z zachowaniem UI GG6; ogólny błąd nie powinien być bez dowodu mapowany na status przeznaczony dla innej klasy.

**Test:** MSG/CHAT i kombinacje flag, ustawiony/nieustawiony bit bez ACK, wejściowy bit QUEUED, blokada i pełna skrzynka, poprawna pusta wiadomość sterująca obrazkiem, brak obrazu, fragmenty i restart w ich trakcie. Żaden taki test nie może opierać się jedynie na encoderze rGG.

## P05 — P2 przed E09/E11 — Nieprzedstawialna treść po przyjęciu nie ma polityki kolejki

**Sekcja:** M05, M11 FIFO, §6.2 kroki 3–5, ryzyko mieszanych wersji.

**Dowód:** wynika bezpośrednio z planu. M05 wybiera odmowę dla znaków niemożliwych do zakodowania. §6.2 przewiduje commit i odpowiedź nadawcy, a dopiero później kodowanie dla klienta. Konta są wspólne dla generacji, zatem odbiorca offline może później zalogować się starszym klientem niż ten, z którym ostatnio korzystał z konta.

**Scenariusz — przyszła możliwość:** serwer przyjmuje tekst Unicode od nowej generacji do odbiorcy offline; odbiorca loguje się klientem CP1250. Odmowa przy odbiorze jest za późna na zwykłe odrzucenie pierwotnego wysłania. Bez osobnego stanu wiadomość może stale wracać na początek FIFO lub zostać usunięta mimo wcześniejszego przyjęcia. To nie dotyczy jeszcze jednorodnego GG6.

**Dokładna poprawka:** przed włączeniem mieszanych generacji wybrać i opisać jedną politykę: ograniczenie przyjmowanej treści do jawnie wspieranego zakresu odbiorcy, świadoma konwersja produktu albo trwałe odłożenie dla zgodnego klienta. Przy odłożeniu określić, czy późniejsze zgodne wiadomości mogą przejść i jak to zmienia FIFO, limity i informację dla użytkownika. To samo dotyczy formatowania, obrazków oraz długości po kodowaniu. `encode_for_client` musi rozróżniać trwałą niezgodność od przejściowego błędu transportu.

**Test:** nieprzedstawialny znak, następnie zwykła wiadomość; zmiana nowy -> stary -> nowy klient między commit a doręczeniem; restart podczas odłożenia. Sprawdzić brak cichej utraty, nieskończonego retry i niezamierzonego zablokowania całej kolejki.

## P06 — P2 — „libgadu” jako test GG6 wymaga przypięcia kompatybilnej wersji

**Sekcja:** §9.1/§9.2, E01/E02, narzędzie niezależnego testu protokołu.

**Fakt i dowód:** odczytany bieżący [`libgadu.c`, `gg_login()`](https://raw.githubusercontent.com/wojtekka/libgadu/master/src/libgadu.c) podnosi `protocol_version < 0x2e` do `0x2e`; nie realizuje w ten sposób logowania GG6. Samo ustawienie `0x20` nie czyni aktualnej biblioteki klientem GG6.

**Scenariusz:** CI opisane jako „test libgadu GG6” wysyła w rzeczywistości inny wariant logowania. Może fałszywie obciążyć poprawny serwer GG6 lub później zaliczyć wyłącznie nowszy adapter.

**Dokładna poprawka:** manifest testowy ma wskazywać konkretną historyczną wersję/commit libgadu z obsługą docelowego profilu oraz konfigurację kompilacji, albo odrębny niezależny klient testowy. Dodać asercję pierwszego pakietu logowania i rzeczywistych pól możliwości. Biblioteka aktualna pozostaje dobrym narzędziem dla obsługiwanych przez nią późniejszych generacji.

**Test:** przechwycenie logowania klienta testowego musi wykazać `GG_LOGIN60` i właściwy numer protokołu dla testu GG6; raport podaje SHA klienta/biblioteki. Native GG6.0 i 6.1 pozostają osobnymi bramkami.

## Obszary ocenione bez dodatkowej uwagi blokującej

- **Adaptery generacji:** §1.3/M04 słusznie nie utożsamiają wersji marketingowej z wire formatem i capabilities. Rozdzielenie GG7, GG7.7 i GG8/10 jest zasadne; brak podstaw, aby już deklarować zgodność. Fixture powinny obejmować rzeczywiste maski możliwości, a nie tylko domyślne wartości.
- **GG32/SHA1 i hasła:** M08 prawidłowo wymaga zmiennego challenge i dostępu do sekretu potrzebnego dla starszego logowania. Przenosząc [poggadaj gg.go:20](D:/Dokumenty/Documents/ChatGPT/Serwery/gg-server-review-2026-10-30/poggadaj/src/internal/security/gg/gg.go:20), należy zachować wejście bajtowe. Jego funkcje przyjmują Go `string`, a [fork hash.rs:7](D:/Dokumenty/Documents/gg-retro/protocol/src/hash.rs:7) iteruje po bajtach Rust UTF-8; zgodność dla polskiego hasła nie wynika z samej zgodności algorytmu. Jest to już objęte zasadą jawnego kodowania w planie, więc nie dodaję osobnej uwagi za istniejące wymaganie. SHA1 należy objąć E09 tam, gdzie rozpoznany klient używa LOGIN70, nie odsuwać automatycznie do E11; [poggadaj GG_Login70.go](D:/Dokumenty/Documents/ChatGPT/Serwery/gg-server-review-2026-10-30/poggadaj/src/cmd/poggadaj-tcp/protocol/GG_Login70.go) pokazuje osobne pole metody i 64-bajtowe pole odpowiedzi.
- **GG6 ACK/FIFO/retransmisje:** M10/M11 nie obiecują niemożliwego exactly-once, nie robią globalnej deduplikacji po sekwencji i jawnie określają kompromis po zapisie TCP. To poprawny punkt wyjścia. Pozostają P03/P04 oraz testowanie zachowania klienta po wynikach nadawczych; sam napis DELIVERED w protokole nie jest dowodem przeczytania.
- **Katalog:** M14 wiernie przenosi znalezione rozbieżności sukcesu, sekwencji i pustych pól z [raportu GG6.1](D:/Dokumenty/Documents/gg-retro/docs/gg6-directory-compatibility.md). Macierz odpowiedzi błędu powinna dodatkowo zachować WRITE=2, READ=4, SEARCH=6 dla sprawdzonego 6.1.0.158. Nie generalizować tego na GG6.0 bez testu.
- **Konferencje:** M15 wymaga ustalenia, kto rozsyła kopie, co chroni przed podwójnym fan-out. Test trzech/czterech uczestników musi liczyć rzeczywiste kopie i sprawdzać listę pozostałych rozmówców u każdego odbiorcy, także offline. Sam zgodny tekst nie zalicza konferencji.
- **Obrazki:** klasyczny model bez trwałego cache jest spójny z D07/M16. Ważny test cyklu życia: po zakończeniu doręczenia wiadomości i usunięciu jej treści oferta nadal musi przez ustalony czas umożliwiać legalne żądanie obrazu; ponowna blokada ma to uprawnienie odebrać. Nie wykazano sprzeczności, bo plan przewiduje osobny `image_offers.expires_at`.
- **DCC/relay:** odrębny dokument badawczy i brak obietnicy pełnej obsługi przed E10 są właściwe. Macierz E07 powinna jasno oznaczać pliki/DCC jako nieobsługiwane, aby „GG6 gotowe” nie zostało odczytane jako pełne wsparcie każdego przycisku klienta. Badania mają osobno objąć legacy DCC i DCC7, reklamowane adresy/porty, połączenie bezpośrednie, relay, odmowę i wznowienie. poggadaj deklaruje te funkcje jako niedziałające, więc nie jest dowodem ich kompatybilności.

## Minimalne uzupełnienie katalogu fixture

Poniższa tabela konkretyzuje istniejącą strategię; nie zastępuje testów planu ani nie potwierdza ich wykonania.

| Obszar | Wymagany niezależny materiał / warunek |
| --- | --- |
| Tożsamość klienta | GG6.0 i GG6.1 osobno: pełny build, SHA256 binarki, opis modyfikacji przekierowujących, OS/locale, sposób uruchomienia i surowa syntetyczna sesja. Dla 6.1.0.158 istnieje identyfikacja w raporcie katalogu, ale nie dowodzi testu rGG. |
| Logowanie | Poprawne/błędne hasło, challenge 0 i wartości z najwyższym bitem, różne losowe challenge, bajty >127 przy zaakceptowanej polityce, obrót GG32 o 0, arytmetyka overflow, NUL i długość graniczna. SHA1: jawna kolejność bajtów seed, rzeczywisty digest i padding; wynik z niezależnej implementacji. |
| Sesja | Pusta lista, wiele fragmentów listy początkowej, PING/PONG i bezczynność przez kilka cykli, timeout, normalne wylogowanie, zastąpienie sesji i restart. Krótkie „login OK” nie zalicza stabilnego połączenia. |
| Kontakty | Długości i przerwania P01; osobno archiwum kontaktów i subskrypcje. Polskie znaki, puste pola, usunięcie listy, pełne przejście eksport -> klient -> import. |
| Wiadomości | Klasy P04, mapowanie statusów ACK i widoczny stan UI, ten sam seq dla różnych treści/nadawców, FIFO przy blokadzie i niezgodnym formacie, pełna skrzynka, reconnect po utracie odpowiedzi nadawczej. |
| Formatowanie | CP1250: polskie znaki przed zmianą atrybutu i obrazkiem; granica offsetu; wiele zmian stylu. Później pary CP1250/Unicode oraz długości liczone po kodowaniu odbiorcy. |
| Katalog | Sukces i błąd wszystkich trzech operacji; losowe/zerowe READ/WRITE seq; sparse profile; usuwanie całego formularza; więcej niż jedna strona; granice roku; widoczność statusu i brak pustego wiersza. |
| Konferencje/obrazy | Cztery osoby, jedna offline i jedna blokująca; dokładna liczba kopii; duplikaty uczestników; opis obrazu plus request/first/more/unavailable; błąd CRC, brak fragmentu, zmiana sesji, retencja oferty po sprzątaniu treści. |
| Transfery E10 | Udane, odrzucone, przerwane i wznowione sesje per generacja oraz różne warunki sieci. Nie zaliczać legacy DCC tylko testem DCC7. |

W fixture wolno używać wyłącznie syntetycznych danych. Wektor znanego hasła testowego jest użyteczny kryptograficznie i nie powinien być zastępowany własnym encode/decode rGG. Surowy capture i opis oczekiwanego efektu w UI powinny być powiązane jednym identyfikatorem scenariusza.