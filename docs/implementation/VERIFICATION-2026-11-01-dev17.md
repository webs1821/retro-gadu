# Weryfikacja lokalna dev.17 — 2026-11-01

Wersja 0.1.0-dev.17, schema 11 bez zmian. Przyrost dotyczy narzędzi pakowania
klienta i testów; kod runtime Rust nie zmienił się poza numerem wersji workspace.
Nie uruchamiano klienta, instalatora ani usług zewnętrznych. Nie było push/deploy.

## Wyniki

- Pięć testów Python: RT_STRING/C-string i końcówki URL/HTML, granice i cykle PE,
  warunki poprawek, NSIS/CRC/dekompresja z limitem, przerwana publikacja i odmowa
  nadpisania oraz konfiguracja. Wszystkie przeszły.
- Oryginalny instalator: trzy hosty (`127.0.0.1`, `gg.example.org`, 16 znaków),
  identyczny stub/skrypt/ładunek poza klientem i nagłówki PE, dokładne zakresy
  poprawek, ABI aktualizatora i nazwy dźwięków; uszkodzony/repatchowany plik
  odrzucony, źródło niezmienione. Wszystkie przeszły.
- Niezależny od parsera odczyt 464 zasobów przez Windows LoadStringW dla każdego
  z trzech klientów odpowiadał tekstom oryginału po zamianie hostów. Tylko odczyt
  zasobów jako danych; nie test wykonania klienta.
- Rzeczywisty lokalny katalog klienta/instalatora został przyjęty przez rGG.
  GET obu plików, SHA-256, HEAD, instrukcje i logowanie syntetycznego klienta
  przez TCP przeszły. Wszystkie procesy testowe zakończone.
- Build workspace dev.17 i test lifecycle (lock, crash/restart, epoka, integralność,
  SIGTERM oraz odmowa publicznego nasłuchu) przeszły.
- Inwentarz: 10 modułów, 190 wersji zależności. RustSec: 0 podatności i 0 ostrzeżeń,
  baza 1395 advisory, commit 2295466c4e9df26647fd67d8fa1c705ac2fc7f81.
  Pełne 77 testów Rust i regresja funkcjonalna były wykonane dla niezmienionego
  kodu runtime w dev.16; nie przedstawia się ich jako nowego uruchomienia dev.17.

Lockfile SHA-256: `f6c46efda4a4a8aa6f40b3702dbdfcaa98f55ff3402f8fefff0209f9c5b11a86`.
Plan pozostał niezmieniony: `cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec`.

## Lokalny wynik

Katalog poza Git: `D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-client-local-2026-11-01`.
Adres klienta `127.0.0.1`. Nie używać do podmiany działającej instalacji GG.

| Plik | Rozmiar | SHA-256 |
|---|---:|---|
| gg.exe | 790528 | ddbc24fc91ffbb5e4bd29ff628436e60a8eb1ca4a7cd617bad3e32910bdf541f |
| gg-setup.exe | 1530043 | e931a700296b325fa86fbd01772d1a98b8d0869cc739bad663d66cf8220a2de7 |

Źródło ma hash wskazany w CLIENT-PACKAGES.md. Klient i instalator nie są kodem
projektu na AGPL. Bez kwalifikacji uruchomienia/natywnego GG, bez testu publicznej
domeny i bez zmiany DNS/hosts. Pozostałe ograniczenia są jawne w PARITY.md.
