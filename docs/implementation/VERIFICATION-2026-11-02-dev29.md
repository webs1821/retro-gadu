# Weryfikacja dev.29 — bufory obrazków

Lokalny Linux/WSL, dane syntetyczne, własne porty loopback. Schemat 13,
190 zewnętrznych wersji bibliotek bez zmian. Nowe porty aplikacji i formatter
metryk nie zmieniają protokołu GG ani limitów transferów.

- 119 testów Rust workspace, formatowanie, Clippy all-targets/all-features
  z `-D warnings` oraz normalny build przeszły.
- Osiem grup procesowych/TCP/HTTP przeszło w 63,954 s: image_metrics,
  gg6_images, http_metrics, runtime_metrics, process_metrics,
  session_contracts, r0_smoke i lifecycle.
- Nowa próba deadline: 30,458 s, 140 migawek, działające pingi i dodatkowy
  fragment po 28 s. Bufory zostały zwolnione. Nie zmieniono deadline 30 s.
- Sprawdzono kompletny poprawny obraz, brak nazwy pliku w metrykach,
  restart w trakcie zbierania i ponowne pobranie, limit czterech transferów,
  oba rozłączenia oraz unavailable przy serwerze bez GG.
- Rust sprawdza także 32 pełne alokacje, stany ready/missing, CRC/overflow,
  read-only i nieblokujące zachowanie migawki, poison, brak fałszywych zer
  oraz 27 rodzin/maks126 szeregów poniżej 16 KiB przy skrajnych liczbach.
- Inwentarz bibliotek, 11 wpisów pochodzenia, granice 10 crate i 13 migracji,
  pierwotny SHA planu oraz `git diff --check` przeszły.

Normalne binarki debug:

- rgg-server: `55e176f6c0e47a97f0f7a8d0061f94ed51dcb466df9e0dd611932d407ff4d26a`
- rgg-admin: `a4d48c5441016ced693458c32817118e0783c7aefced41293a199fae62872ac5`

Raport [finalny](image-metrics-2026-11-02-dev29-final.json) zawiera wyniki,
sumy źródeł i binarek oraz hash [pierwszej próby](image-metrics-2026-11-02-dev29.json).
Pierwsza próba zakończyła się błędnym oczekiwaniem nowego testu: quota miała
już poprawnie zwracać ACK4 bez missing, a test oczekiwał ACK6 i missing.
Skorygowano tylko asercję testu. Udane kontrole Rust/build zachowano i powiązano
po ponownym sprawdzeniu sum wszystkich źródeł runtime oraz binarek; nie
uruchamiano ich drugi raz wyłącznie w celu powtórzenia logów.

Test dołączono do CI i przyszłego container_suite (31 grup). Nie zbudowano
nowego obrazu ani nie powtórzono pełnego R0; r0_smoke to tylko 6 sesji i
10 wiadomości. Ostatni kwalifikowany obraz pozostaje dev.26. Brak kwalifikacji
natywnego klienta, długiego soak, push, wdrożenia i zmian działającego gg-retro.
Procesy testowe zakończone, tymczasowe dane usunięte przez sterowniki.
