# Weryfikacja rGG 0.1.0-dev.12 — 2026-11-01

Linux/WSL, Rust 1.99.1. 69 testów Rust, fmt, Clippy wszystkich celów/features
bez ostrzeżeń. Schemat 8, 10 modułów, 190 wersji zależności. Bez nowych zależności
zewnętrznych; admin korzysta z istniejących sha2 i hex.

`backups_cli.py`:

- Pełny backup i immutable verify niezmieniający plików; 0700/0600.
- Restore do nowego katalogu, logowanie, oczekująca wiadomość z obrazkiem,
  pobranie obrazu od nadawcy, profil, archiwum kontaktów, reguły kontaktów,
  rezerwacja usuniętego UIN, wykorzystane zaproszenia i zachowanie keyringu.
- Odmowa istniejącego celu/kopii, aktywnego serwera i nieistniejącego źródła.
- Odmowa hash mismatch, braku/złego klucza, uszkodzonego sekretu drugiego konta
  przy poprawnym pierwszym, złej checksumy migracji i złamanego klucza obcego
  (także po ponownym obliczeniu hasha manifestu), WAL, symlinka i zbyt szerokich praw.
- Dwa process-exit 86: przed publikacją manifestu i usunięciem znacznika restore.
  Verify odrzuca niekompletną kopię, serwer i admin blokują niepełny katalog.
  Ponowienie do innego celu działa.

Przeszły regresje accounts_cli i lifecycle (blokada, recovery, migracje,
readiness, SIGTERM, odmowa publicznego bind). Roundtrip wykonuje również
rzeczywiste TCP GG6 do syntetycznego klienta. Końcowy zwykły build bez hooków.
Nie powtarzano wszystkich niezmienionych testów HTTP z dev.11 ani natywnego GG6.

[RustSec](cargo-audit-2026-11-01-dev12.json): 0 podatności i 0 ostrzeżeń.
Cargo.lock SHA-256: `f6cfde231fb075e3255c3d5c0c72667509d48db177b0833dacda8a4c559b9e4a`.
Inwentarz, kontrola repo, GG32 i diff poprawne; plan i recenzje niezmienione.
Wyłącznie dane syntetyczne, bez push i wdrożenia. Ograniczenia w [BACKUPS](BACKUPS.md).
