# Weryfikacja rGG 0.1.0-dev.15 — 2026-11-01

Linux/WSL, Rust 1.99.1, **74 testy Rust**, fmt i Clippy wszystkich celów/features
bez ostrzeżeń. Schemat 10, nowe receipt/metadane importu; wcześniejsze migracje
niezmienione. 10 modułów i 190 wersji zewnętrznych bibliotek (base64 już był w lock).

Importer: trzy syntetyczne konta z niezależnymi wektorami libsodium, pending tekst
i obrazek z konferencją, terminalny nagłówek bez payloadu, polski profil i jego
wyszukiwanie po płci/imieniu, prywatne nazwy, archiwum, opis/prywatność, dwa
zaproszenia ze stanem, licznik numerów i dodatkowa rezerwacja. Po imporcie działa
logowanie GG6, dokładne FIFO/timestamps, pobranie obrazka oraz backup/restore.
Stary kod zużywa wyłącznie ostatnie pozostałe użycie. Hash źródła nie zmienia się.

Odmowa bez utworzenia celu: zły klucz, długość i uprawnienia, symlink, zamieniony
szyfrogram konta, plaintext, brak potwierdzenia luki historii, duplikaty, kolizja
rezerwacji, sieroty, niekodowalny tekst, błędne opcje/CRC, konflikt z blokadą,
nieznane pole JSON, złe liczniki i nadmierna kolejka. Sprawdzono CLI wrappera.

Trzy awarie exit(86): przed utworzeniem SQLite, przed commit i po commit przed
publikacją. Pozostaje marker, dane są odpowiednio nieobecne/0/3 konta, zwykłe CLI
i serwer odmawiają startu. Existing-target retry odmawia, nowy katalog przechodzi.
To test awarii procesu, nie fizycznego zaniku zasilania. Końcowy build bez hooków.

Binarki dev.14 utworzyły rzeczywisty schemat 9 z zaproszeniem użytym 2 razy,
kontami, rezerwacją UIN, katalogiem, kontaktami i oczekującym obrazkiem.
Migracja do 10 zachowała dane i doręczenie. Restore starej kopii narzędziem
dev.14 do nowego katalogu, a potem migracja nowym adminem również przeszły.
SHA-256 odniesienia: server `a2395e37508cc75896f7c955d9924855cbe807a90f66287436bc6727370791ff`,
admin `7551785231a7a84747826c007d91ab8831f3bd86be0c97ccb3afa0b56c7ff12c`.

Przeszła pełna zwykła regresja: lifecycle, accounts_cli, gg6_sessions,
presence_contacts, directory, conferences, images, http_discovery, registration,
statistics, downloads, backups_cli, keys_cli, legacy_invitations i inspection.
Po dopracowaniu najwcześniejszego markera powtórzono Rust, importer/3 awarie,
lifecycle i backup/restore. Starszych hooków awarii sesji/kopii/rotacji nie
powtarzano — mają zapisane wyniki w poprzednich raportach.

[RustSec](cargo-audit-2026-11-01-dev15.json): 0 podatności, 0 ostrzeżeń;
1395 advisory, ostatni commit bazy 2295466c4e9df26647fd67d8fa1c705ac2fc7f81.
Cargo.lock SHA-256: `e03a61d988a4b03867e984d95681676b87c1428d28328e19f3a004641acf720f`. Plan pozostaje niezmieniony.
Kwalifikacja nie obejmuje prawdziwej bazy, klienta natywnego ani wdrożenia.
