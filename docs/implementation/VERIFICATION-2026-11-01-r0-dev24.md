# Obraz dev.24 — powtórzony dłuższy R0

Źródło: **bf94fbd78d813f1a64e9192ba59dafebde4968b9**, schema12.
Obraz zawiera poprawkę kolejności zapisów z dev.23 oraz SQLite3.51.3.
Parametry ustalono przed próbą, zgodnie z niezmienionym R0-LOAD.md:
600 sekund,100 sesji Ready,10 wiadomości/s,1CPU,256MiB RAM,swap0,128PID.

## Wynik

Wszystkie bramki przeszły. Przyjęto6000 wiadomości; niezależny klient TCP
sprawdził3000 online i3000 offline, pełne payloady oraz kolejność offline.
ACK p95=4.535ms, maksimum15.326ms; online p95=9.180ms, maksimum15.219ms.
Średni CPU fazy pomiaru11.5566% pojedynczego rdzenia. Maksymalne RSS procesu
18522112B (17.66MiB), pamięć całej grupy20459520B (19.51MiB). Bez swapu/OOM.

Po czystym SIGTERM: integralność/FK prawidłowe, zero sesji i przechowywanych
payloadów;6000 rekordów TransportWrittenUnconfirmed. Ta nazwa pozostaje istotna:
protokół GG6 nie potwierdza przeczytania przez człowieka ani zapisu w kliencie.
Tymczasowe kontenery i wolumeny zostały usunięte oraz sprawdzone.

Przed R0 dokładny obraz przeszedł26 zestawów release w86.664s i osobną próbę
trwałego wolumenu, restartu, wyłączności CLI, backup/verify/restore i ponownego
uwierzytelnienia/zapisu po restore. Te dwie próby miały faktyczny swap=max;
raporty to ujawniają. R0 dodatkowo ustawił i sprawdził swap0 we własnej grupie.

## Identyfikatory i archiwum

- Runtime: sha256:054d8dd16f12e3a08cf11bc5aaf3a65064704f7c8c89cff8fea3b1a48eb7c016.
- Verification: sha256:6ee934bb355ecd7ff47909b4b12e2d1d806a5373dd5c6623412e2ed11d6dfcb0.
- rgg-server: 68faf007173ab6667905e181d4617843714d85129a7a71309f0c887ac6f1609a.
- Komplet źródeł w obrazie: 60ca04f9e4c39145115e1db4b7ed76a9ef1d64cf7f4332a4b9960bfb87cc280f.
- rgg-dev24-runtime.tar.gz:61680178B,
  SHA25695088150316b620c0812c19350981bcc05d1050bb6d925bb451f6df005432d37.

Eksport jest w workspace/rgg-container-2026-11-01-dev24. Zweryfikowano gzip/tar,
manifest, hash konfiguracji obrazu i etykietę commitu. Raport
container-artifact-2026-11-01-dev24.json łączy sumy artefaktu, źródła, sterowników
testowych i sześciu surowych raportów. Poprzednie archiwa i nieudane próby dev22
zachowano. Prywatny daemon Docker zatrzymano po eksporcie, z zachowaniem cache.

To udana próbka R0, nadal nie długi soak, kwalifikacja całego natywnego GG6 ani
publicznego wdrożenia. Nie wykonano push/deploy. Wynik dotyczy wskazanego obrazu,
nie późniejszych zmian recepty klienta.
