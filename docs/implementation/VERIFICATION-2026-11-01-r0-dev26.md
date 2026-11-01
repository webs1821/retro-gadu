# Obraz dev.26 — kwalifikacja lokalna

Źródło: **51c078cc8a5e2c2a329791fa4ef4ccf31b514ed5**, schema 13.
Obraz zawiera poprawkę klienta GG6.1, indeks i zapytania pełnej historii oraz
poprawiony sterownik przygotowania sztucznej bazy. Wcześniejsza próba obrazu
nie przeszła z powodu pozostawionego połączenia/WAL w fixture; zachowano diagnozę
SQLITE_FULL w container-dev26-attempt1.json. Limity nie zostały zwiększone.

## Wyniki dokładnego obrazu

27 zestawów release przeszło w 84,912 s. Osobno sprawdzono trwały wolumen,
restart, wyłączność CLI, backup/verify/restore i uwierzytelnienie oraz zapis po
odtworzeniu. Faktyczny swap tych prób wynosił max, co zapisano w raportach.
R0 dodatkowo ustawił i sprawdził swap 0 we własnej grupie cgroup.

Parametry zapisano przed R0: 600 s, 100 Ready, 20 kont offline, 10 wiadomości/s,
1 CPU, 256 MiB, swap 0, limit 128 PID. Przyjęto i sprawdzono wszystkie 6000
wiadomości: 3000 online i 3000 offline, dokładne payloady i FIFO.

- ACK p50 3,981 ms, p95 5,431 ms, maksimum 21,548 ms.
- Dostarczenie online p95 10,389 ms, maksimum 28,197 ms.
- Średnie CPU fazy pomiarowej: 11,9971% jednego rdzenia.
- Maksymalne RSS procesu: 18849792 B, około 18 MiB.
- Szczyt pamięci cgroup: 20975616 B, około 20 MiB.
- Czas 600,001 s, tempo 10/s, maksymalne opóźnienie planowania 5,133 ms.

Wszystkie bramki przeszły. Po SIGTERM integralność/FK były poprawne, sesje
i treści wiadomości usunięte, a 6000 rekordów miało stan
TransportWrittenUnconfirmed. GG6 nie daje potwierdzenia odbioru/zapisu w kliencie.
Sprzątanie kontenerów i wolumenów zostało wykonane oraz sprawdzone.

## Tożsamość i archiwum

- Runtime: sha256:1a6f76583208ce19a9aad4ba9f1d51cab803fbb21dc63658c860da6678fcbb59.
- Verification: sha256:3f0806df9532b2c448ee7f547b3060cdd6079d230a590a150334a42f05bf7e59.
- Serwer: 9a83c9885ca7a0888d0e086a17d1c8ebbf4476a3c6ea444a7b045aee90d7c053.
- Archiwum źródeł w obrazie: 285a8a912537541a5c0134142c582c6538f1aed96f2f38cb4ac26852fa970484.
- rgg-dev26-runtime.tar.gz: 61911804 B,
  SHA256 e07315106bd55e8c87c2aef72477bda9e923400d0a3fb55b9cebc90ec9c39381.

Eksport jest w workspace/rgg-container-2026-11-01-dev26. Sprawdzono gzip do EOF,
manifest i hash konfiguracji obrazu oraz etykietę commitu. Sześć surowych raportów
i container-artifact-2026-11-01-dev26.json zapisano obok archiwum i w repozytorium.
Prywatny daemon zatrzymano po eksporcie; cache i poprzednie artefakty zachowano.

To próbka R0, nie długi soak, test całej aplikacji GG6 ani kwalifikacja publicznego
wdrożenia. Brak push/deploy i zmian działającego gg-retro, klienta czy DNS/hosts.
