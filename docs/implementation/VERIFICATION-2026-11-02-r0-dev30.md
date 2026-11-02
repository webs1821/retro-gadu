# Obraz dev.30 — kwalifikacja lokalna

Źródło: **94730efddb6c39b9d6e47b3d50b1f1114994ea9e**, schema 13.
Obraz obejmuje metryki CPU/RSS i obrazków, jawny adres discovery oraz profil
WWW za lokalnym proxy HTTPS. Runtime nadal nasłuchuje wyłącznie na loopback.

## Wyniki dokładnego obrazu

32 zestawy release przeszły w 123,254 s. Oddzielny test sprawdził trwały wolumen,
restart, wyłączność CLI oraz backup/verify/restore, logowanie i zapis po restore.
Źródła testów są tylko do odczytu w `/src`; tmpfs 32 MiB zawiera dane tymczasowe.
Pierwsza próba miała ENOSPC podczas przygotowania bazy 100000 wpisów, jeszcze
przed startem serwera: źródła zajmowały 5414912 B na tym samym tmpfs co baza/WAL.
Diagnozę i nieudaną próbę zachowano w `container-dev30-attempt1.json`.
Poprawiono rozmieszczenie plików testowych, zachowując runtime i wszystkie limity.

Parametry R0 zapisano przed uruchomieniem: 600 s, 100 Ready, 20 kont offline,
10 wiadomości/s, 1 CPU, 256 MiB, swap 0, limit 128 PID. Wszystkie 6000 wiadomości
przyjęto i sprawdzono: 3000 online i 3000 offline, dokładne payloady i FIFO.

- ACK p50 4,049 ms, p95 5,217 ms, maksimum 34,366 ms.
- Dostarczenie online p95 9,910 ms, maksimum 24,690 ms.
- Średnie CPU fazy pomiarowej: 11,84% jednego rdzenia.
- Maksymalne RSS: 19136512 B, około 18,25 MiB.
- Szczyt pamięci cgroup: 21266432 B, około 20,28 MiB.
- Czas 600,002 s, tempo 10/s, maksymalne opóźnienie planowania 4,707 ms.

Wszystkie bramki przeszły. Po SIGTERM integralność i klucze obce były poprawne,
nie było sesji ani treści wiadomości, a 6000 rekordów dostarczeń miało stan
`TransportWrittenUnconfirmed`. GG6 nie potwierdza odbioru/zapisu w kliencie.

Proces miał UID 10001, root filesystem tylko do odczytu, brak capabilities,
no-new-privileges i filtr seccomp. Docker w WSL początkowo pozostawił swap `max`;
sterownik R0 ustawił 0 dopiero po kontroli własności cgroup i sprawdzał go w próbkach.
Wcześniejsze suite/restart/restore miały swap `max`, co zachowano w ich raportach.
Kontenery i wolumeny testowe usunięto i sprawdzono. Prywatny daemon zatrzymano
po eksporcie, zachowując cache, obrazy i wcześniejsze archiwa.

## Tożsamość i archiwum

- Runtime: sha256:79bfa28fec6565f9638b75cd0295e71a76a083fcbfaba2ea7defb9e45c13d2ce.
- Verification: sha256:3396ab839fee98b30cb37e6d7a3747973a84dd5716df3eb5695fad75440f19a6.
- Serwer: 56ca062d39d465d976409ec9b1ffc210514cf6eec020583c1a8bff9f58611768.
- Źródła w obrazie: 9df021b562ff156817ac195f3a1d1cb85de7ffbe04e61ed64a8114cd55e74f3a.
- `rgg-dev30-runtime.tar.gz`: 62104705 B,
  SHA256 2fd69c37b1c65cb9024ff5f131e45d7f91b59e04f3293a4367c22488b42fc2b7.

Eksport znajduje się w workspace `rgg-container-2026-11-02-dev30`.
Sprawdzono gzip do EOF, manifest, hash konfiguracji i etykietę commitu obrazu.
Sześć surowych raportów i `container-artifact-2026-11-02-dev30.json` są obok
archiwum oraz w repozytorium; manifest wiąże je sumami SHA256.

To próbka R0, nie długi soak, wykonanie całego GG6, test nginx ani kwalifikacja
publicznego wdrożenia. Nie wykonano push, wdrożenia ani zmian działającego GG.
