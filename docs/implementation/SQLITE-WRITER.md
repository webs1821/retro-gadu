# SQLite: jeden zapisujący i osobne odczyty

Dev.23 zachowuje łącznie najwyżej cztery połączenia, WAL, synchronous=FULL,
foreign_keys=ON, blokadę katalogu, schemat 12 i wszystkie terminy transportu.
Jedno połączenie obsługuje zapisy; trzy pozostałe są otwierane z read_only=true
i query_only=ON. Pula SQLx przydziela połączenia w kolejności oczekiwania.
Oczekujące zapisy nie zajmują połączeń potrzebnych do odczytu.

W poprzedniej organizacji cztery połączenia mogły jednocześnie próbować zapisu.
SQLite szereguje piszących, ale rywalizacja odbywała się już wewnątrz workerów
biblioteki, z busy_timeout=5s, podczas gdy transport ma 2s na claim i zapis TCP.
Nie zwiększono żadnego z tych terminów. Zapisujące transakcje, rejestracja,
katalog, hasła, nonce, kontakty, wiadomości, klucze oraz VACUUM INTO używają
teraz puli zapisującej. Transakcje snapshotów statystyk/metryk pozostają odczytowe.
Inicjalizacja i migracje kończą się przed otwarciem puli odczytowej.
Zamknięcie obejmuje obie pule, nadal pod blokadą katalogu danych.

Źródła kontraktu: [SQLx 0.8.6 Pool — FIFO acquire](https://docs.rs/sqlx/0.8.6/sqlx/struct.Pool.html)
oraz [SQLite — współbieżność WAL](https://www.sqlite.org/wal.html#concurrency).
Nie kopiowano implementacji z tych dokumentów.

## Odtworzenie błędu i poprawka

- Obraz dev.22, źródło a5d55d37ba29d00b7772629b9d6ce99452a0f1a9:
  25 zestawów release i osobny test restartu/wolumenu/backup-restore przeszły.
- Przed dłuższym R0 zapisano plan: 600s, 100 Ready, 6000 wiadomości, CPU1,
  RAM256MiB, swap0, PID128. Nie zmieniono progów R0. Nadawanie zakończyło się,
  ale odbiór 3000 offline zakończył się EOF; pełna próba NIE przeszła.
- Diagnostyka samej kolejki 3000: sukces w 15.519s. Dodanie 3000 zakończonych
  dostarczeń odtworzyło EOF; metryki wskazały delivery timeout i zamknięcie
  jednej sesji. Użyto tych samych limitów kontenera i prawdziwego TCP.
- Kandydat z jedną pulą zapisującą odebrał wszystkie 3000 offline w 22.516s,
  przy 3000 już zakończonych dostarczeń. Wszystkie 20 sesji pozostały połączone.
  Po SIGTERM: 6000 TransportWrittenUnconfirmed, zero sesji i payloadów,
  integrity_check=ok i brak błędów FK.

Kandydat release miał jeszcze etykietę dev.22 podczas pracy, SHA256
cd3dafadcfbd373f1c2e8e98339287793661db592b8ac224819f345d39f047ce.
Był uruchomiony z osobnego testowego wolumenu w bazowym obrazie dev.22.
To diagnostyka poprawki, NIE kwalifikacja nowego obrazu ani zakończony R0.
Wcześniejsza próba debug nie ukończyła przygotowania historii przy nadawaniu
100/s; dopisano jawne bariery Ready i zakończenia dostarczeń przygotowujących
fixture. Limit fazy odtwarzania pozostał 30s. Nie odrzucono nieudanych raportów.

Test Rust sprawdza postęp odczytów przy zablokowanym zapisie, kolejność dwóch
oczekujących transakcji, rollback po drop i odrzucenie zapisu przez czytnika.
tests/gg6_concurrent_replay.py przygotowuje istniejącą syntetyczną historię
offline, a następnie sprawdza 20 klientów TCP, dokładne payloady, FIFO,
stan bazy po zapisie oraz SIGTERM. Nie udaje testu przyjmowania wiadomości
ani kwalifikacji wydajności: osobny R0 obejmuje oba kierunki i limity zasobów.

## Otwarta aktualizacja biblioteki

Przy sprawdzaniu dokumentacji producenta potwierdzono, że obecne
libsqlite3-sys 0.30.1 kompiluje SQLite 3.46.0. Producent opisuje
[błąd resetu WAL](https://www.sqlite.org/wal.html#the_wal_reset_bug), poprawiony
w 3.51.3 i nowszych (także w wybranych backportach). Aktualizacja biblioteki
i ponowne testy pozostają wymagane przed końcową kwalifikacją. Nie utożsamiamy
tej rzadkiej usterki integralności z odtworzonym timeoutem: w opisanej próbie
nie stwierdzono uszkodzenia bazy. Wersja sqlite3 z kontrolnego Pythona w raporcie
nie jest wersją SQLite wbudowaną w binarkę rGG.

Wszystkie bazy i kontenery były lokalne i syntetyczne. Nie było push,
publikacji obrazu ani zmian działającego gg-retro.
