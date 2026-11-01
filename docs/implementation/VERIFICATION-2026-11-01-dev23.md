# Dev.23 — współbieżne odtwarzanie kolejek

Wersja 0.1.0-dev.23, schema12. Jedno połączenie zapisujące SQLite, maksymalnie
trzy odczytowe; nadal WAL/FULL, te same limity transportu i niezmienione migracje.
[Kontrakt i odtworzony błąd](SQLITE-WRITER.md).

Weryfikacja na Linux/WSL z Rust1.99.1:

- 101 testów Rust przeszło; fmt oraz Clippy all-targets/all-features -D warnings.
- 23 grupy TCP/HTTP/CLI przeszły, w tym rzeczywista migracja kopii dev20/schema11
  do schema12, rejestracja, oba sposoby zmiany hasła, import, rotacja i restore.
- 9 grup --faults przeszło: sesje, kontakty, konferencje, obrazki, hasło WWW,
  kopie, klucze, importer i hasło CLI. Przywrócono binarki bez test-hooks.
- Nowy test kolejki odtwarza 3000 wiadomości do 20 klientów przy 3000 wpisach
  historii. Sprawdza payload/FIFO, brak timeoutów i stan po SIGTERM. Dodany do CI
  i przyszłych suite kontenera; zwykły test debug nie kwalifikuje wydajności.
- Kontrola repo: 9 wpisów pochodzenia, 10 modułów, 12 migracji, 190 zależności.
  Hash oryginalnego planu pozostał cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec.

Surowe wyniki: sqlite-writer-regression-2026-11-01.json oraz
sqlite-writer-faults-2026-11-01.json. Osobna diagnostyka kandydata release
przy CPU1/RAM256MiB/swap0/PID128: replay-2026-11-01-writer-release.json.
Odebrano 3000 offline w22.516s; wszystkie6000 wiadomości były już wcześniej
przyjęte przez rzeczywisty TCP. Po zamknięciu zero sesji/payloadów i poprawne FK.

Nieudany dłuższy R0 obrazu dev22 jest zachowany w r0-2026-11-01-dev22-failed.json,
a parametry ustalone wcześniej w r0-2026-11-01-dev22-plan.json. Raporty
replay-2026-11-01-dev22-* zachowują wynik kolejki bez historii i odtworzenie EOF
z historią. Zachowano także próbę debug z nieukończonym przygotowaniem historii
oraz próbę rozpoczętą przed zakończeniem budowy (bez uruchomionego kandydata).
Kontrolny Python raportuje własne SQLite3.40.1; binarka rGG ma wbudowane3.46.0.

Nowy pełny R0, aktualizacja SQLite z poprawką WAL-reset, nowy obraz, długi soak,
natywny GG6 i profil publiczny pozostają otwarte. Nie było push ani wdrożenia.
