# Weryfikacja rGG 0.1.0-dev.6 — 2026-11-01

Lokalny Linux/WSL, Rust 1.99.1. 52 testy Rust przeszły; formatowanie i Clippy
dla wszystkich celów i funkcji bez ostrzeżeń. Test HTTP potwierdził discovery
wyłączone/włączone, prawdziwy port loopback, odporność odpowiedzi na Host i
X-Forwarded-*, HTTP/1.0 bez Host, zakończenie po odpowiedzi i logowanie GG6 pod
adresem otrzymanym przez HTTP.

Trzy stare endpointy operacji kont odmówiły bez zmiany liczby kont, zużycia
zaproszenia ani zmiany hasła istniejącego konta. Parametr z syntetycznym sekretem
nie trafił do logu. Nowe ścieżki GG8 pozostały niedostępne. Testy cyklu życia
i pięć grup sesji/kolejki TCP również przeszły. Regresje katalogu i kontaktów
mają pełny wynik z dev.5; ten przyrost zmienia HTTP i konfigurację.

Kontrola pochodzenia: 4 wpisy, 9 modułów, 6 migracji. Nie zmieniono wersji
zewnętrznych bibliotek: 190 zinwentaryzowanych wersji, [RustSec](cargo-audit-2026-11-01-dev6.json)
bez znanych podatności i ostrzeżeń. SHA-256 Cargo.lock:
`e0fe698273a79b38ccd349b087f374ac4b2312ca2c4195ccc20c736584977f5f`.

Brak kwalifikacji natywnego GG/Windows. Nie zmieniano DNS, hosts, istniejącego
klienta ani produkcji. Plan i recenzje bez zmian. Lokalny workflow rozszerzony,
bez push i uruchamiania GitHub Actions. [Kontrakt discovery](HTTP-DISCOVERY.md).
