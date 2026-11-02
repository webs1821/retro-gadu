# Dev.33 — kontrola prywatności katalogu danych

Schemat14, wersja0.1.0-dev.33. Serwer i CLI odrzucają na Unix istniejący katalog
z uprawnieniami dla grupy/innych użytkowników, zanim otworzą blokadę lub bazę.
Kontrolują typ plików SQLite/lock, dowiązania i uprawnienia blokady.
Nie naprawiają istniejących danych automatycznie. Nowe katalogi pozostają0700.

## Dowód błędu i testy

W dev32 zainicjalizowano wyłącznie syntetyczne konto w istniejącym katalogu0755.
Oddzielny proces z UID65534 odczytał adres syntetyczny z bazy0644; klucza0600
nie mógł odczytać. [Dowód](private-directory-2026-11-02-dev32-failure.json).
Testy nowej wersji odrzucają ten układ przed stworzeniem bazy/blokady.
Kontener32 używał0700 i nie miał tej ekspozycji.

- 131 testów Rust: PASS, pełny stdout w raporcie. Pierwsza próba ujawniła,
  że wcześniejsze fixture tempfile tworzyły katalogi z szerszymi uprawnieniami.
  Testy tworzą teraz jawnie0700; kontrakt runtime nie został poluzowany.
  Zachowano również log pierwszego niepowodzenia.
- Formatowanie, Clippy all-targets/all-features z -D warnings i build: PASS.
- 10 grup procesu/CLI/TCP: PASS w21.182s. Prywatność i25 przypadków
  dowiązań/FIFO/katalogów, konta, backup/restore, klucze, zmiana i reset haseł,
  importer, hasła WWW, sesje i lifecycle. Migracja rzeczywistej bazy dev31
  do14 i reset nadal poprawne. Istniejąca baza nie zmienia się po odmowie.
- Audyt RustSec odświeżony dla lockfile33:0 podatności i0 ostrzeżeń.
  [Pełny wynik](cargo-audit-2026-11-02-dev33.json). Inwentarz190 zewnętrznych
  wersji odświeżony i sprawdzony; nie zmieniono żadnego rekordu biblioteki.
- Plan bez zmian. Schemat14 nie wymaga nowej migracji. Nowy test dodano do CI
  oraz kontenerowego zestawu (teraz34grupy).

## Binarne artefakty debug

- rgg-server: `e87990579aa028c969da74f024ab0213bee0e1be7e7aa91b32334638a78d752d`
- rgg-admin: `36bf755635abcb958137612e386da23258b0e49360bd9e6b24eb5084ec149c9d`

Poprzedni debug zachowano w /tmp/rgg-baseline-dev32. [Raport JSON](private-directory-2026-11-02-dev33.json)
wiąże hashe źródeł i binariów z wynikami. Syntetyczne testy TCP działały w
prywatnej przestrzeni sieciowej/user namespace, tylko loopback.

Nie kwalifikowano ACL Windows ani ochrony przed właścicielem/root. Starsze
pliki SQLite0644 są dopuszczone wewnątrz prywatnego katalogu0700. Należy
zabezpieczyć także ścieżkę nadrzędną. [Pełny kontrakt](DATA-DIRECTORY.md).
Obraz33 i pełny soak pozostają kolejnym krokiem. Brak push i wdrożenia.
