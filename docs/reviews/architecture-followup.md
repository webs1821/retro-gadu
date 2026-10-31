# Retro Gadu — ponowna kontrola uwag architektonicznych

Data: 2026-10-31. Dokument oceniony: PLAN_IMPLEMENTACJI.md v0.2, 837 wierszy, SHA-256 `54ba707b2958f3dc3fe85984e3a4f1157bbfc33fe8b8a714c2c5ea4c241298b2`.

Zakres ograniczono do rozstrzygnięcia A01–A09 z pierwszego raportu oraz zgodności tabeli ROZLICZENIE_UWAG.md z planem. Oceniono specyfikację i przypisane bramki, bez wymagania już istniejącego kodu lub wyników przyszłych testów. Nie zmieniono planu, kodu ani pierwszego raportu i nie korzystano z produkcji.

**Ocena: plan jest gotowy do rozpoczęcia E01.** Poprawki usuwają zasadnicze luki wskazane w pierwszej rundzie. Wybór jednego procesu, właściciela stanu per konto, ograniczonego koordynatora, warunkowych operacji magazynu oraz oddzielnego writera jest teraz spójnie opisany. Pozostaje jedno niewielkie uzupełnienie tabeli stanów dla przyszłych profili z ACK; nie blokuje ono E01 ani zakresu GG6 bez ACK odbiorcy.

## Rozliczenie A01–A09

| Uwaga | Ocena specyfikacji v0.2 | Podstawa |
| --- | --- | --- |
| A01 | Zamknięta specyfikacyjnie. | §3.2 określa punkty skuteczności, atomowe sprawdzenie wersji/uprawnień i odrzucenie wyniku starego uwierzytelnienia. M08–M09 i §9.3 wymagają odpowiednich przeplotów. |
| A02 | Zamknięta specyfikacyjnie. | §3.3 obejmuje nierozpoczęte ramki, pozostały czas lease, częściowy zapis, barierę i callback bez cyklu oczekiwania. §3.4 dodaje ProcessEpoch i rozdziela zegary. Testy są nazwane w §9.3. |
| A03 | Zamknięta specyfikacyjnie. | M11 definiuje FIFO aktywnej kolejki i atomowy enqueue_order. M13 terminalizuje treści odrzucone przez blokadę; wznowienie DeferredForCapability trafia na koniec, zachowuje accepted_at/expiry i liczy się do limitów. Informacja w WWW jest warunkiem funkcji. |
| A04 | Istota uwagi zamknięta; pozostało uzupełnienie P2 opisane niżej. | M11 i §5 rozdzielają deliveries od historii prób, wymuszają bieżącą własność, dopuszczają wczesne ACK i zabraniają cofania stanu terminalnego. Korelacja istnieje przed I/O. Brakuje tylko jawnego przejścia ponowienia z AwaitingClientAck. |
| A05 | Zamknięta specyfikacyjnie. | §3.4 wymaga systemowej wyłączności katalogu, a M21 rozdziela CLI online przez serwer od offline z wyłącznością. E01–E02 mają odpowiadające bramki. |
| A06 | Zamknięta specyfikacyjnie. | §3.4 definiuje porty atomowych przypadków użycia; M10 i M16 wymagają wspólnej transakcji wiadomości oraz ofert. M19 obejmuje reset/outbox, a §10.1 przypisuje kontrakt E03 i test pełnego bundle E06. |
| A07 | Zamknięta specyfikacyjnie. | M20 wybiera WAL/FULL, sprawdzenie puli i lokalny trwały wolumen. Rozróżnia awarię procesu, systemu/zasilania oraz utratę nośnika. §11.3 określa rolę kopii/RPO. |
| A08 | Zamknięta specyfikacyjnie. | §3 wskazuje application::contracts jako właściciela Command/Event/DTO, jednokierunkową zależność protokołu i adapter jako jedynego autora bajtów. E01 obejmuje kontrolę rzeczywistego grafu. |
| A09 | Zamknięta specyfikacyjnie. | M09 i E02 wprowadzają minimalny trwały kontrakt prywatności oraz Ready przed doręczaniem; allow-all pozostaje wyłącznie narzędziem testowym. E04 rozwija istniejącą granicę. |

Koordynator poza pętlami kont i zakaz oczekiwania actora na wynik innego actora rozstrzygają wcześniejszy kontrprzykład obustronnej rozmowy. §3.3 wprost rozstrzyga także cykl actor → join writera → callback do tego actora. Nie ma podstaw do utrzymywania tych zastrzeżeń jako otwartych na poziomie planu.

## Pozostałe doprecyzowanie: A04-R1 — P2

**Miejsce:** M11, tabela modelu wyników oraz tabela legalnych przejść, wiersze 319 i 333–343.

**Luka:** opis AwaitingClientAck przewiduje ponowienie według profilu po restarcie, ale tabela legalnych przejść dopuszcza powrót do Pending wyłącznie z Leased. Nie opisuje przejścia lub wymiany próby po timeout/braku ACK w stanie AwaitingClientAck.

**Scenariusz:** zapis TCP oraz stan AwaitingClientAck są utrwalone, ACK nie dociera i proces jest uruchamiany ponownie. Dosłowna implementacja zamkniętej tabeli przejść nie może uruchomić zapowiedzianej ponownej próby albo dopisze ją poza tym kontraktem.

**Dokładne uzupełnienie:** dodać w tabeli `AwaitingClientAck → Pending` po timeout/recovery przewidzianym przez profil, z potwierdzeniem utraty starego prawa do I/O, atomowym CAS i zamknięciem korelacji poprzedniej próby. Następne przejęcie tworzy nowy token i stosuje zbadaną dla profilu regułę wire_seq; nie odnawia expiry i nie reaktywuje stanów terminalnych. Alternatywny model ponownej próby bez pośredniego Pending też jest poprawny, jeśli zostanie jawnie zapisany z tymi samymi warunkami.

**Bramka:** dopisać przejście przed implementacją obsługi odbiorczych ACK, najpóźniej w E11. W specyfikacji scenariusza timeout/restart mają istnieć legalna droga ponowienia i wynik opóźnionego ACK starej próby. Nie wymagam uruchomienia tych testów przed rozpoczęciem E01.

Tabela rozliczenia trafnie opisuje zasadnicze zmiany. Przy A04 warto odnotować powyższe uzupełnienie zamiast interpretować „uwzględniono” jako całkowitą kompletność tabeli przejść. Innych konkretnych pozostałości A01–A09 na poziomie planu nie stwierdzono. Przyszłe badania protokołu i wykonanie testów pozostają właściwie przypisanymi zadaniami etapów, a nie blokadą startu fundamentu.

## Zaakceptowana decyzja po przekazaniu dodatku

Autor przyjął A04-R1 i zobowiązał się dopisać w wersji finalnej `AwaitingClientAck → Pending` po timeout/restart: utrata starego prawa do I/O/bariera, CAS stanu nieterminalnego, unieważnienie starej korelacji, zachowany enqueue_order oraz nowy token i zgodny z profilem wire_seq przy ponownym przejęciu. Jeśli ACK wcześniej wygrało CAS i ustawiło ClientAcked, ponowienie jest niedozwolone. Decyzję należy stosować przy niezmienionym expiry zgodnie z ogólną polityką retencji.

**Ta decyzja zamyka A04-R1 na poziomie uzgodnionej specyfikacji.** Wersja 0.2 wskazana powyższym hashem nadal dokumentuje stan przed dopisaniem tego wiersza; niniejszy dodatek odnotowuje akceptację treści poprawki, a nie niezależne ponowne odczytanie finalnego pliku. Po jej naniesieniu nie pozostaje otwarte rozstrzygnięcie architektoniczne A01–A09. Ocena gotowości rozpoczęcia E01 jest pozytywna; nie jest potrzebna trzecia pełna runda przeglądu ani wcześniejsze pisanie kodu/testów tylko w celu zatwierdzenia planu.
