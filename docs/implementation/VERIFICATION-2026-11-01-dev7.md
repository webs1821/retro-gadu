# Weryfikacja rGG 0.1.0-dev.7 — 2026-11-01

Linux/WSL, Rust 1.99.1: 55 testów Rust, formatowanie i Clippy wszystkich
celów/funkcji bez ostrzeżeń. Zwykły build bez test-hooks. Cztery grupy testów
HTTP: numer własny/automatyczny i login GG6 po restarcie, Origin/Host/CSRF oraz
niepoprawne dane, równoczesne rejestracje, deadline i limit rzeczywistego IP.
12 konkurujących żądań wykorzystało zaproszenie dokładnie 3 razy; integrity_check OK.
Nieudane próby nie zużyły zaproszenia. Nowe konta nie pojawiły się samoczynnie
w katalogu. Hasła/kody/e-mail testowy nie wystąpiły w logach.

Przeszły również discovery (2 grupy), katalog (3), obecność/kontakty (4), sesje
i kolejka (5), CLI i cykl życia. Kontrola wyglądu: przeglądarka desktop i lokalna
baza tymczasowa, bez prawdziwych danych. Nie kwalifikowano GG6 ani Windows native.

9 modułów, 6 migracji, 4 wpisy pochodzenia. Wersje zewnętrznych bibliotek
niezmienione: 190. [RustSec](cargo-audit-2026-11-01-dev7.json): 0 podatności
i 0 ostrzeżeń. Cargo.lock SHA-256: `e753666cc76780b1f0b7cea10e01af9e110015063b259545a288cc85005b26c2`.

Plan i recenzje bez zmian; brak push, zdalnego repo i wdrożeń. HTTP wyłącznie
loopback. Kontrakt i ograniczenia opisuje [WEB-REGISTRATION.md](WEB-REGISTRATION.md).
