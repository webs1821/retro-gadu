# Weryfikacja dev.19 — prywatne metryki

2026-11-01, Windows + WSL Linux, Rust 1.99.1, własne sztuczne konta/bazy.
Schema 11 bez zmian, 10 modułów i 190 wersji zależności. Zależności produkcyjne
nie rozszerzyły się; async-trait/test-util dodano tylko do testów HTTP.

## Wyniki

- `cargo check --workspace --offline`, `cargo fmt`, pełne testy: 88 passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed.
- Store: current epoch/generation/credential/active/presence, sesje niewidoczne,
  disabled/retired/puste konta, queued→leased→written→retention, przyszły czas,
  bajty UTF-8 archiwum i wygasłe granty; zamknięta baza daje błąd.
- HTTP: deterministyczne testy gotowości, cache/expiry/błędu bazy, limitu dwóch
  żądań i deadline 1 s. Realne TCP/HTTP: stałe 12 rodzin/19 szeregów, nagłówki,
  osobne routery, brak danych użytkowników, ukrywanie niewidoczności w publicznych
  stats przy prywatnym liczniku sesji, trwała kolejka po SIGKILL i zmiana stanu
  po wysłaniu; retirement; Origin/Host/fetch-site/duplikaty, body/query/metody.
- Cztery częściowe nagłówki metryk nie blokują zwykłego health ani GG; SIGTERM
  kończy wszystkie listenery w ograniczonym czasie.
- Regresja: `lifecycle.py`, `gg6_sessions.py`, `http_statistics.py`,
  `http_limits.py`: passed. Po rozszerzeniu testów skrajnych store ponownie
  sprawdzono dwa testy tego modułu oraz pełny Clippy.
- Plan zachował SHA256 `cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec`.

Normalna binarka debug WSL po `cargo build --workspace --locked`, bez test-hooks:
`93ce00f9303922c0030b3fb00d970cd78e7992f7148e811f3243d10e1a1368c5`.

## Granice dowodu

Format jest zgodny ze sprawdzoną specyfikacją i parserem testowym; nie
uruchamiano Prometheusa. Metryki są agregatami bazy, nie pełną telemetrią
procesu/protokołu. Brak nowego obrazu dev.19 — wcześniejsze wyniki obrazu
dev.18 pozostają przypisane do jego SHA i starego sterownika. Nowa pozycja
http_metrics w sterowniku kontenera nie jest jeszcze testem obrazu dev.19.
Nie było push, wdrożenia, testów na rzeczywistych danych ani zmian klienta GG.
