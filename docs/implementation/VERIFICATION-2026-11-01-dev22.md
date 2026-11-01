# Weryfikacja dev.22 — 2026-11-01

Dodano stałe liczniki procesu i histogramy logowania, przyjęcia i dostarczania,
współdzielone przez transport GG i prywatny endpoint metryk. Brak zmian schematu
12, formatu pakietów, polityki ACK, zależności i zasad dostępu.

## Wyniki

- **100 testów Rust**, fmt i Clippy all-targets/all-features z `-D warnings`: OK.
  Nowe przypadki obejmują progi histogramów, saturację, jednorazowe zakończenie,
  spójny odczyt z czterech wątków, maksymalny rozmiar eksportu oraz anulowanie
  zadania posiadającego rzeczywisty TCP socket, sygnał fence i obserwację login.
- **16 grup regresji**: lifecycle, runtime metrics, dotychczasowe metrics,
  HTTP limits/stats/password, sesje, obecność/kontakty, katalog, konferencje,
  obrazki, discovery, rejestracja, downloads, patcher, smoke sterownika R0.
  [Pełne wyniki](runtime-metrics-regression-2026-11-01.json) zachowują również
  pierwotny błąd czytnika metryk; najnowszy wynik każdej grupy jest poprawny.
- Pierwszy smoke zatrzymał się, bo stary czytnik dopuszczał tylko 8 KiB.
  Limit odczytu podniesiono do zweryfikowanych 16 KiB. Nie zmieniono progów
  ACK/latencji/RSS/CPU ani parametrów obciążenia. Końcowy smoke sprawdza 6 sesji
  i 10 wiadomości przez 5 s; **nie jest kwalifikacją pełnego R0**.
- Dodatkowy slow-reader test wykazał, że timeout może nastąpić podczas claim,
  zanim lease dotrze do transportu. Pierwsza wersja liczyła dopiero po lease;
  końcowa obejmuje claim i pomija tylko zakończone puste poll. W tej próbie
  sprawdzono licznik timeout, zwolnienie socketu, brak pozostawionego Leased,
  wpis Interrupted oraz Pending. Pozostały klient nadal odpowiada na PING.
- Po korekcie powtórzono Rust/Clippy i końcowe runtime_metrics/http_metrics/
  r0_smoke na zwykłych binarkach; cztery scenariusze runtime przeszły.
- **5 grup z test-hooks**: sesje, obecność/kontakty, konferencje, obrazki,
  hasło WWW. Wszystkie przeszły, także granice commit/TCP/restart.
  [Surowe wyniki](runtime-metrics-faults-2026-11-01.json).
- Kontrola repo: 9 rejestrów pochodzenia, 10 modułów, 12 migracji, 190 bibliotek.
  Inwentarz zaktualizowany do Cargo.lock dev.22; nowe biblioteki nie były potrzebne.

## Znaczenie i ograniczenia

Dokładne zakresy operacji i stałe kategorie opisuje [kontrakt](RUNTIME-METRICS.md).
Endpoint ma 17 rodzin i 112 szeregów. Histogram +Inf równa count; wynik i czas
operacji są zapisywane w jednej krótkiej sekcji krytycznej. Testy zabraniają
eksportu syntetycznych haseł, treści, e-maili, opisów i etykiet identyfikujących.
Nie zmieniono ochrony Host/Origin ani limitów osobnego listenera.

Nie uruchamiano Prometheusa/promtool i nie twierdzimy, że licznik success
potwierdza odczyt wiadomości przez człowieka lub klienta. Wynik anulowany może
odpowiadać zakończonemu commit. Restart zeruje runtime, zachowując kolejkę DB.
Zamknięty GG6, profil TLS/proxy, długi soak i pełna kwalifikacja produkcyjna
pozostają otwarte. Obraz/R0 nadal dotyczą dev.19, nie tej rewizji.

## Artefakty

WSL, Rust 1.99.1. Normalny build bez test-hooks przywrócony po fault tests:

| Plik | SHA-256 |
|---|---|
| `/tmp/rgg-target/debug/rgg-server` | `3aaf15b9e2ed74e88b7e03c64f4b7e3801da68cb776337b89184753e823ff381` |
| `/tmp/rgg-target/debug/rgg-admin` | `ae75e03328d52b937e3d8fc610ed2b59fa7606d12f8253f4c8f12a0ea8cec9d4` |

Plan nadal SHA-256 `cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec`.
Wszystkie dane i porty były lokalne, wszystkie procesy prób zakończone.
Nie wykonano push ani nie zmieniono działającego gg-retro.
