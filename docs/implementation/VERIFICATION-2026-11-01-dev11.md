# Weryfikacja rGG 0.1.0-dev.11 — 2026-11-01

Linux/WSL, Rust 1.99.1. **69 testów Rust** przeszło. Formatowanie i Clippy
wszystkich celów/funkcji bez ostrzeżeń. Schemat SQLite pozostaje 8.

Cztery grupy `http_downloads.py` przeszły: pełny GET i HEAD, długość/SHA-256,
bezpieczny HTML i nagłówki, JSON bez ścieżek lokalnych, niezmienność po podmianie,
odmowa startu przy błędnych hashach, manifestach, rozmiarach, duplikatach i symlinkach,
wyłączone endpointy, niezależne włączenie, Host/Fetch-Site/źródłowy IP, Range 416,
cztery wolne strumienie oraz odzyskanie slotu po zamknięciu połączenia.
Logowanie GG i health działają również przy zajętych wszystkich slotach pobierania.

Przeszły regresje rejestracji (4 grupy), statystyk (3), discovery (2), obrazków
(5) i lifecycle. Zestaw Rust obejmuje także niezmienione repozytoria, sesje,
obecność, katalog i transfery. Nie powtarzano w tym przyroście całej kwalifikacji
awarii SQLite z dev.10, ponieważ kod persystencji nie został zmieniony.

Podgląd desktop w IAB: sprawdzono stronę z kartą klienta i instalatora,
instrukcją podmiany, przyciskami i informacją o osobnych warunkach używania.
Oba pliki były sztucznymi bajtami testowymi. Podgląd i proces zamknięto, dane
tymczasowe usunięte przez helper. Test mobilny i prawdziwego instalatora pozostają otwarte.

190 zewnętrznych wersji zależności, bez nowej wersji biblioteki. HTTP używa już
obecnych bytes, futures-util, sha2 i toml. 10 modułów, 8 migracji, 4 wpisy pochodzenia.
[RustSec](cargo-audit-2026-11-01-dev11.json): 0 podatności, 0 ostrzeżeń.
Cargo.lock SHA-256: `d961cf46cca52d0b2626017185b5e4c275852f5b876b2e31e0ba8622a1ce0e3c`.
Kontrole źródeł, GG32, inwentarza i diff poprawne. Plan i recenzje niezmienione.

Zwykły build bez test-hooks. Wyłącznie lokalnie, bez push i wdrożenia.
Nie dołączono prawdziwych EXE ani patchera rGG; to nadal osobna pozycja PARITY.
Ograniczenia timeoutu wolnego HTTP i publicznego profilu opisuje [kontrakt](DOWNLOADS.md).
