# Dev.32 — reset zapomnianego hasła administratora

Wersja `0.1.0-dev.32`, schemat SQLite 14. Implementacja resetu jest lokalna,
bez nowych zależności zewnętrznych (190 wersji w lockfile). Polecenie wymaga
oryginalnych kluczy i pełnej, zweryfikowanej kopii przed zapisem. Rozróżnia
reset od uwierzytelnionej zmiany hasła w audycie.

## Wyniki

- 128 testów Rust: 0 błędów, w tym cztery nowe testy resetu i audytu.
  Wynik pochodzi z zakończonego wywołania narzędzia; pełnego stdout tego
  uruchomienia nie archiwizowano. Raport oznacza to jawnie.
- Formatowanie, Clippy wszystkich targets i wszystkich features z `-D warnings`,
  kompilacja workspace oraz końcowy build bez `test-hooks`: PASS.
- 13 zestawów CLI/TCP/HTTP: PASS, łącznie 37.529 s. Reset, zmiana hasła
  CLI i WWW, backup/restore, rotacja kluczy, metryki, HTTPS proxy, sesje,
  lifecycle, stare zaproszenia, import, konferencje i obrazki.
- Reset zweryfikowany przez rzeczywiste syntetyczne logowania GG6 i odtworzenie
  kopii sprzed i po zmianie. Zachowane profil, archiwum kontaktów, zaproszenia,
  prywatność, wiadomości i uprawnienia do obrazków. Limit prób wyczyszczony
  wyłącznie dla wskazanego konta.
- Rzeczywiste binaria dev.31 utworzyły bazę schematu 13: migracja do 14 zachowała
  13 poprzednich checksum i wpisy audytu obu kanałów. Sprawdzono również pustą
  tabelę po retencji: nowe ID wynosi 3, nie 1. Stara kopia odtworzona przez
  dev.31 i następnie zmigrowana działa poprawnie.
- Trzy punkty kontrolowanego przerwania resetu: przed publikacją manifestu,
  przed commit i po commit. Integralność, FK, nonce, wersja, generacja, limit,
  audyt oraz stare/nowe logowanie zgodne. Osobno przeszły cztery wcześniejsze
  punkty awarii zmian hasła CLI/WWW. Finalne binaria nie mają haków testowych.
- Pochodzenie i granice modułów: 13 rekordów, 10 crate, 14 migracji. Plan ma
  niezmienny SHA256 `cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec`.

Pierwsza próba nowego testu zakończyła się w przygotowaniu danych: systemowe
SQLite Pythona 3.10 nie ma funkcji `unixepoch()`. Fixture używa teraz przenośnego
`strftime`, bez zmian w runtime SQLite 3.51.3. Błąd i pełny udany wynik są
zachowane w [raporcie JSON](password-reset-2026-11-02-dev32.json).

## Artefakty i zakres

- Serwer debug: `e5fa5b389e9ea1f6060161a3854663f057e63809e8681b7078cce2e7a3e3ac84`.
- Admin debug: `c3f7025072661e2dd73c05ffd345c9c289bf5fb0dbec939823e51369f8c1a7c9`.
- Oryginalne binaria dev.31 zachowano w `/tmp/rgg-baseline-dev31`; ich sumy
  oraz sumy źródeł, buildów, wyniki i polecenia są w raporcie JSON.
- Testy sieciowe działały w prywatnej przestrzeni sieciowej/user namespace
  WSL/Linux, tylko loopback, z tymczasowymi syntetycznymi danymi.
- Scenariusz resetu dodany do CI i `container_suite.py` (teraz 33 zestawy).
  Nowego obrazu nie zbudowano w tym kroku. Ostatni kwalifikowany obraz i R0
  pozostają dev.30; nie obejmują poprawki protokołu dev.31 ani resetu dev.32.

Nie kwalifikowano natywnego GG6, Windows ani długiego soak. Brak push,
wdrożenia i zmian działającego gg-retro. [Obsługa resetu](PASSWORD-RESET.md).
