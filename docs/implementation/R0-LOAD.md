# Scenariusz obciążenia R0 — kryteria przed próbą

Scenariusz zapisany przed wykonaniem pełnej próby. To krótka kwalifikacja
konkretnego obciążenia, oddzielna od długiego soak, testów natywnego GG6,
testów odporności na złośliwe pakiety i zgody na wdrożenie.

## Obciążenie i sprawdzanie danych

- Własny nowy wolumen i 120 sztucznych kont, nigdy dane produkcyjne.
- 100 równocześnie gotowych sesji GG6 przez całe 180 s pomiaru.
- Harmonogram 10 wiadomości/s, 1800 przyjęć. Tekst 128 B, naprzemiennie do
  zalogowanego odbiorcy i do jednego z 20 kont offline. Puste listy subskrypcji.
- Każdy ACK musi oznaczać przyjęcie, każde dostarczenie dokładnie zachować
  nadawcę, sekwencję i payload. Po pomiarze odłączenie 100 klientów, logowanie
  20 odbiorców offline i sprawdzenie całej kolejki/FIFO oraz usunięcia payloadów.
- Osobny klient obciążający w innym cgroup; współdzieli wyłącznie prywatną
  przestrzeń sieciową runtime. Brak sieci zewnętrznej i publikacji portów.
- Prywatne metryki próbkowane raz na sekundę; limity network w tej konfiguracji
  wynoszą 128 globalnie i na IP, ponieważ generator pochodzi z jednego adresu.
  Domyślne limity kolejki pozostają niezmienione.

## Progi ustalone przed wynikiem

| Warunek | Próg |
|---|---|
| Przyjęcia i payloady online/offline | 100%, brak duplikatów i błędów |
| Gotowe sesje w fazie pomiaru | 100 w każdej próbce |
| Rzeczywista szybkość nadawania | co najmniej 9,9/s |
| Maksymalne opóźnienie harmonogramu generatora | 250 ms |
| Opóźnienie ACK p95 | 250 ms, każde ACK przed timeout 1 s |
| Opóźnienie dostarczenia online p95 | 500 ms |
| RSS procesu, także poza samą fazą nadawania | maksymalnie 192 MiB |
| CPU runtime w fazie pomiaru | średnio maksymalnie 90% jednej vCPU |
| Twarde limity runtime | 1 CPU, 256 MiB RAM, swap 0, 128 PID |
| OOM/oom_kill i zakończenie procesu | brak OOM; SIGTERM i exit 0 |

R0 nie przechodzi, gdy którykolwiek próg lub warunek izolacji nie jest
spełniony. Nie zmieniamy progów po zobaczeniu wyniku. Poprawkę implementacji
i kolejną próbę opisujemy osobno. `r0_smoke.py` (6 sesji/5 s) sprawdza tylko
generator i nie może zakwalifikować R0.

Wcześniejszy WSL nie zastosował żądanego limitu swapu Docker. Próba musi
odczytać faktyczne pliki cgroup, nie same opcje CLI. Jeśli konieczne, harness
może ustawić `memory.swap.max=0` wyłącznie w cgroup sprawdzonego, własnego
tymczasowego kontenera. Zmiana i odczyt zwrotny trafiają do raportu. Bez
potwierdzenia limitu wynik nie kwalifikuje R0.
