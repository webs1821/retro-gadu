# Weryfikacja lokalna dev.18 — 2026-11-01

Wersja 0.1.0-dev.18, schema 11 bez zmian. Nowy listener ogranicza gniazda HTTP
przed parsowaniem nagłówków i egzekwuje terminy podczas I/O odpowiedzi.
Nie zmieniono adresów nasłuchu, schematu bazy ani działającego gg-retro.

## Testy i kontrole

- 83 testy Rust: wszystkie przeszły, w tym sześć nowych (walidacja konfiguracji,
  globalne/per-IP przyjęcie i sprzątanie, dwukierunkowy postęp, bezwzględny czas,
  niezależne wybudzenie zablokowanego zapisu i pusty flush).
- Clippy wszystkich modułów, targetów i features z `-D warnings`: poprawnie.
- Trzy testy prawdziwych gniazd HTTP: limit globalny i rzeczywistego peer-a,
  idle niekompletnego nagłówka, nieprzedłużalny lifetime przy stałym podawaniu
  bajtów, zwolnienie czterech slotów pobrania 16 MiB bez zamykania klientów,
  niezależne logowanie/ping GG oraz ograniczone zamknięcie procesu.
- Regresja: lifecycle, discovery (także HTTP/1.0), rejestracja/CSRF/konkurencja,
  liczniki/prywatność, pobieranie i niezmienność snapshotu, rzeczywisty pakiet
  dev.17, sesje/logowanie/FIFO/restart, prywatność kolejki i wolny odbiorca GG.
  Wszystkie przeszły na zwykłych binarkach, bez test-hooks.
- RustSec: 0 podatności i 0 ostrzeżeń; 1395 advisory, commit
  `2295466c4e9df26647fd67d8fa1c705ac2fc7f81`. Inwentarz nadal 190 wersji zależności,
  10 modułów. `tokio/test-util` jest tylko zależnością testową do kontrolowanego czasu.

Podczas pracy test oddzielnych zadań odczytu/zapisu ujawnił przejęcie wybudzenia
przez wspólny timer. Zawieszony test zatrzymano, a implementację poprawiono na
osobne rejestracje timerów z tymi samymi terminami. Końcowy test przechodzi;
przerwanego uruchomienia nie liczy się jako sukcesu.

Cargo.lock SHA-256: `70db6a1796b458fdbb4a7586531dc99f49ccd41245155567464cc0ed218a6d1a`.

## Próba powtarzalna

```sh
python tests/local_endurance.py --server target/debug/rgg-server --admin target/debug/rgg-admin --seconds 180 --report /path/to/new-report.json
```

Próba na Linux/WSL używa trzech syntetycznych kont, naprzemiennych serii online
i offline po 25 wiadomości, cyklicznego logowania odbiorcy, dwóch równoległych
klientów HTTP (health i plik 1 MiB) oraz porzucanych niekompletnych nagłówków.
Każda wiadomość jest porównana i czeka się na usunięcie dostarczonych treści z DB.
Odpowiedź 429 katalogu po wyczerpaniu budżetu jest oczekiwanym ograniczeniem.
Raport zawiera hash zwykłego buildu debug, próbki RSS/HWM/wątków/fd/CPU, opóźnienia
ACK, integralność i wynik zamknięcia. Próg 256 MiB jest kopertą bezpieczeństwa
tej małej próby, nie zaleceniem RAM dla wdrożenia.

Wynik [180-sekundowej próby](endurance-2026-11-01-dev18.json): 4175 poprawnie
porównanych wiadomości w 167 cyklach, 180 pełnych pobrań i 540 oczekiwanych
odpowiedzi 429. ACK p50 4,809 ms, p95 7,798 ms. Proces zużył 60,79 s CPU
w czasie 180,638 s ściennych. RSS wzrósł z 16832 do 24720 KiB (szczyt 24,1 MiB),
liczba wątków 27 → 29, deskryptorów 18 → 22. Baza poprawna, treści dostarczone
usunięte, SIGTERM zakończony poprawnie. Nie stwierdza się na tej podstawie plateau
pamięci: test jest krótki, a SQLite i alokator mogą rozbudowywać cache.

Nie jest to godzinny soak, benchmark maksymalnej wydajności, test natywnego GG
ani kwalifikacja reverse proxy. Nie wyznacza przydziału RAM/CPU w Proxmoxie.
