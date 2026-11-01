# Dev.25 — poprawka zakończenia uploadu kontaktów

[Kontrakt i zakres](CLIENT-UPLOAD.md). Schema 12, 10 modułów Rust i 190 wersji
bibliotek Cargo bez zmian funkcjonalnych serwera. Zależności Rust są identyczne
z audytowanym dev.24; zmieniono tylko wersje lokalnych modułów w lockfile.

- 102 testy Rust, fmt i Clippy all-targets/all-features -D warnings.
- 79 przypadków izolowanej funkcji CPU i 8 przechwyconych sekwencji przez TCP,
  z zachowaniem poprzedniej listy przed końcem oraz restartem i kontrolą bazy.
- 5 testów parsera/instalatora, 3 adresy, 464 zasoby Windows, 6 porównań JS/Python.
- http_patcher.py,http_downloads.py i gg6_presence_contacts.py przeszły;
  client-regression-2026-11-01-dev25.json zawiera pełny wynik i hashe sterowników.
- Rzeczywisty pakiet z osobnego katalogu przeszedł client_downloads.py: identyczne
  bajty plików, SHA256, GET/HEAD, instrukcje oraz równoczesne logowanie GG.
- Plan niezmieniony: cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec.

Raporty CPU/TCP mają osobne identyfikatory wejścia, klienta i programu testowego.
Walidator CPU działa poza programem serwera. Nie uruchomiono natywnej aplikacji,
instalatora, push ani deploy. Obraz i R0 dev.24 pozostają osobnym, historycznym
dowodem dla dokładnego commitu bf94fbd78d813f1a64e9192ba59dafebde4968b9.
