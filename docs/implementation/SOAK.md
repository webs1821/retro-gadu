# Godzinny test stabilności GG6 — kryteria przed próbą

Ten scenariusz rozwija R0 i sprawdza powtarzalne obciążenie jednego procesu
z rosnącą historią w tej samej bazie. Samo istnienie sterownika lub krótka próba
nie oznacza ukończenia testu. Raport wiąże wynik z konkretnym obrazem i commitem.

## Stałe warunki

- Cztery cykle po 900 s nadawania, bez restartu serwera i bez czyszczenia bazy:
  łącznie 3600 s fazy obciążenia, plus logowania, odbiór offline i przerwy.
- W każdym cyklu 100 gotowych sesji, 20 odbiorców offline i 10 wiadomości/s,
  połowa online i połowa offline. Payload128 B, niezależne, niepowtarzane
  numery sekwencyjne między cyklami. Łącznie 36000 dokładnie sprawdzonych kopii.
- Po każdym cyklu rozłączenie nadawców, logowanie odbiorców offline, sprawdzenie
  pełnego FIFO, opróżnienie payloadów i pięć sekund bez sesji. Zatrzymany ruch
  pozwala porównać liczbę deskryptorów, wątków i RSS w tych samych warunkach.
- Limity runtime jak R0: 1 CPU, 256 MiB RAM, swap0, 128 PID, UID/GID10001,
  read-only root, bez capabilities, no-new-privileges, seccomp, brak zewnętrznej
  sieci i publikacji portów. Generator działa w osobnym kontenerze/cgroup.
- 120 kont syntetycznych w nowym wolumenie. Ustawienia kolejki i retencji
  domyślne; max4500 oczekujących kopii/cykl i36000 historii nie przekracza limitów.
- Po zakończeniu SIGTERM/exit0, kontrola integralności i FK, brak sesji,
  payloadów i aktywnych prób, dokładnie36000 TransportWrittenUnconfirmed.

## Progi ustalone przed wynikiem

Wszystkie bramki R0 muszą być spełnione w każdym cyklu: 100% ACK i payloadów,
brak błędów/duplikatów,100 Ready w każdej próbce, co najmniej9,9 wiadomości/s,
opóźnienie harmonogramu max250ms, ACK p95≤250ms i każde<1000ms,
online p95≤500ms, CPU średnio≤90% jednej vCPU w danej fazie pomiaru.

Przez cały test RSS≤192MiB, swap0 i brak OOM/oom_kill. Co sekundę próbkowane są
rzeczywiste liczniki procesu i cgroup. Okres pomiaru musi pokrywać każdy cykl
z tolerancją najwyżej3s na granicach próbkowania.

Dodatkowe bramki stabilności, porównujące ostatnie3s pierwszej i ostatniej
przerwy bez sesji:

| Obserwacja | Dopuszczalna zmiana |
|---|---|
| Mediana RSS | wzrost najwyżej16MiB |
| Mediana liczby otwartych deskryptorów | wzrost najwyżej4 |
| Mediana liczby wątków | wzrost najwyżej4 |
| Próbki w każdej przerwie | co najmniej2 |
| Proces runtime | ten sam PID/starttime przez całą próbę |

Wzrost pliku bazy z historią jest zamierzony; nie jest usuwany dla poprawienia
wyniku. Weryfikator kopiuje stabilną bazę i ewentualny WAL z wolumenu read-only
do osobnego, własnego wolumenu roboczego, z limitem64MiB; nie pomija WAL przez
immutable=1. Wszystkie zasoby testu są oznaczone i sprzątane według własności.

## Zakres wyniku

To godzinny test wskazanego ruchu tekstowego, kolejek i ponownych połączeń.
Nie sprawdza tygodniowego TTL, wielodniowych wycieków, mediów pod ciągłym
obciążeniem, rzeczywistej sieci/WAN, klienta natywnego ani odporności na wszystkie
rodzaje ataków. Nie upoważnia do wdrożenia. Krótkie warianty służą wyłącznie
weryfikacji sterownika i nie ustawiają `soak_qualified=true`.

Przykład pełnej próby po zbudowaniu i sprawdzeniu aktualnych obrazów:

```sh
python tests/container_load.py --docker /path/to/docker --socket /path/to/private.sock \
  --runtime sha256:RUNTIME --verification sha256:VERIFICATION \
  --seconds 900 --cycles 4 --report /path/to/new-soak-report.json
```

Próg uznania pełnej próby: cztery cykle po co najmniej900s, wszystkie bramki
oraz potwierdzone sprzątnięcie zasobów. Oryginalny plan pozostaje niezmienny.

## Pierwsza weryfikacja sterownika

`r0_smoke.py` przeszedł dwa cykle5s na jednym procesie i bazie:6sesji,
20wiadomości, po5online i5offline/cykl, historia0→10→20 i sekwencje1–20.
To27,064s całego scenariusza, tylko10s nadawania. Raport:
[soak-driver-smoke-2026-11-02.json](soak-driver-smoke-2026-11-02.json).
Krótki test obejmuje generator i runtime debug; sterownik kontenera i pełne
progi wymagają dalszej próby. Dodane później sprawdzenie aktywnych prób dostawy
w weryfikatorze kontenera nie jest objęte tą próbką.
