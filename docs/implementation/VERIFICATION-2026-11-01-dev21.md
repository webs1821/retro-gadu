# Weryfikacja dev.21 — 2026-11-01

Zmiana: opcjonalny formularz pełnego obecnego hasła, serializacja przez aktora
konta, zatrzymanie starego transportu przed zmianą sekretu, wspólna transakcja
CLI/WWW oraz migracja 11→12. Rozwój wyłącznie lokalny, bez push i wdrożenia.

## Wyniki

- **95 testów Rust**: wszystkie przeszły; także fmt i Clippy ze wszystkimi
  targetami/features oraz `-D warnings`. Normalny build bez test-hooks odtworzony.
- **21 grup regresji** zwykłych binarek: lifecycle, konta, kopie, klucze,
  hasło CLI, sesje, obecność/kontakty, katalog, discovery, rejestracja, hasło WWW,
  stare zaproszenia/import, statystyki, downloads, patcher, limity HTTP,
  metryki, krótki smoke R0, konferencje i obrazki. Każda zakończona kodem 0.
  [Surowy wynik](http-password-regression-2026-11-01.json) zawiera czasy i stdout.
- Testy Rust wymuszają jednocześnie barierę socketu i trzymaną publikację,
  przekazanie Leased→Pending/Interrupted, odrzucenie starego zapisu transportu,
  nowy lease oraz odrzucenie starego dowodu logowania bez wyrzucania nowej sesji.
- Trwałe 5 prób/600 s, restart/cofnięty zegar, nieznany numer bez alokacji,
  konkurencyjne zmiany dokładnie raz, powtórzony nonce z pełnym rollback.
- Realne HTTP/GG6: błędne i niezmienione hasło nie zamykają klienta; poprawne
  zamyka stary transport. Nowe logowanie działa, stare odrzucone; profil,
  archiwum, prywatność, zaproszenia i granty obrazków zachowane. Pending wraca
  po logowaniu nowym hasłem; poprawne obrazy są nadal przekazywane.
- Sprawdzone niezależne cele CSRF, replay, Host/Origin, zły cookie, body 413,
  query, zakres UIN, potwierdzenie/ASCII, opt-in oraz blokada `keys.pending`.
- Proces z test-hooks kończy się kodem 86 przed i po commit. Odpowiednio stare
  albo nowe hasło, zgodne nonce/wersja/audyt oraz integralność DB po restarcie.
- Rzeczywista dev.20 z commitu `18e3a1a` utworzyła schema11. Migracja zachowuje
  stare rekordy i najwyższe ID także po skasowaniu ostatniego albo wszystkich
  wpisów. Z 2 przechodzimy do ID3; stare archiwum pozostaje niezmienione.
  Restore przez starszą binarkę, a następnie migracja i nowe logowanie przeszły.
- Przeglądarka desktop: obejrzano górę i dół formularza; nie wpisywano haseł
  ani nie wysyłano formularza przez UI. Zapis sprawdzono niezależnym klientem
  HTTP na kontach syntetycznych. Podgląd i karta zamknięte.
- Kontrola repozytorium: 9 wpisów pochodzenia, 10 granic modułów, 12 migracji.
  Nadal 190 wersji bibliotek, brak nowych zależności lub zapożyczonego kodu.

## Odtworzenie

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --locked
python tests/http_password.py --server target/debug/rgg-server --admin target/debug/rgg-admin
cargo build --workspace --locked --features rgg-server/test-hooks,rgg-admin/test-hooks
python tests/http_password.py --server target/debug/rgg-server --admin target/debug/rgg-admin --faults
# Opcjonalnie: katalog zachowanych binarek dev.20, schema11:
python tests/http_password.py --server target/debug/rgg-server --admin target/debug/rgg-admin --baseline-dir /tmp/rgg-baseline-dev20
cargo build --workspace --locked
```

CI obejmuje zwykłe i awaryjne testy formularza. Nie zakłada obecności starszej
binarki: rzeczywista migracja z dev.20 jest osobno wykonanym wynikiem lokalnym.
Scenariusz przyszłej kwalifikacji kontenera rozszerzono o test WWW hasła.

## Artefakty i granice

WSL, Rust 1.99.1, zwykłe debug binarki:

| Plik | SHA-256 |
|---|---|
| `/tmp/rgg-target/debug/rgg-server` | `74aa1363596bb0ba7da369c4600af6198f87ff03ca5a3553e6b2c67de46242c8` |
| `/tmp/rgg-target/debug/rgg-admin` | `5bf73c0ddba683ea1f89ed17e51a9c0501b49ee68f4a8853885051135f02edf2` |

Plan nadal SHA-256 `cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec`.
Zrzuty UI w `D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-browser-local-2026-11-01`:
`password-form-dev21.jpg` i `password-form-dev21-footer.jpg`. Nie ma sekretów.

Nie kwalifikowano oryginalnego klienta GG ani TLS/proxy. Brak resetu hasła
bez znajomości starego. Nie wykonano nowego obrazu ani pomiaru R0 dla dev.21:
obraz i pełna krótka próbka pozostają oznaczone **dev.19**. Utrata odpowiedzi
HTTP może oznaczać zatwierdzoną zmianę, a granica TCP/DB może powtórzyć wiadomość;
nie deklarujemy dokładnie jednego dostarczenia do klienta.
