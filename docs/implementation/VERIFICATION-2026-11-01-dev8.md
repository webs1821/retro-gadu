# Weryfikacja rGG 0.1.0-dev.8 — 2026-11-01

Linux/WSL, Rust 1.99.1. 56 testów Rust przeszło. Formatowanie oraz Clippy
wszystkich celów/funkcji bez ostrzeżeń; zwykły build bez test-hooks.

Nowy test magazynu obejmuje statusy, opis niewychodzący do agregatu, tylko
znajomych, active, ready, session_id, generation, credential_version i epoch.
Sprawdza także odłączenie, retirement oraz nową epokę. Licznik kont obejmuje
wyłączone konta, a nie obejmuje tombstone. Trzy grupy HTTP/TCP sprawdzają
gotowość początkową, prywatność, wymianę sesji bez podwojenia, restart, rejestrację
WWW, niezależne opt-iny, Host, pochodzenie i wspólne limity rzeczywistego IP.

Przeszły regresje rejestracji WWW (4 grupy), discovery (2) i cyklu życia.
Pozostałe testy TCP i CLI mają pełny przebieg z dev.7; kod ich zachowania nie
zmienił się. Stronę główną obejrzano w przeglądarce desktop na tymczasowych
danych; bez testu nowego widoku na urządzeniu mobilnym ani kwalifikacji GG6.

Inwentaryzacja 190 wersji bibliotek, bez zmian wersji zewnętrznych.
[RustSec](cargo-audit-2026-11-01-dev8.json): 0 podatności, 0 ostrzeżeń.
Cargo.lock SHA-256: `5772ebada602fe034e37a901b29c114af62debf1c1db681cd7e2f9ce628bd295`. Kontrola pochodzenia, GG32 i diff poprawna.
Plan i recenzje niezmienione. Brak push, produkcji, DNS i zmian rzeczywistego GG.
