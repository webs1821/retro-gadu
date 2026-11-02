# Dev.31 — opisy GG6 i niezależny klient libgadu

Poprawiono opisy bez NUL w LOGIN60 i NEW_STATUS. Błąd rozłączenia odtworzono na
dev.30 z zewnętrzną libgadu 1.7.0; raport niepowodzenia pozostaje w
`libgadu-client-2026-11-02-dev30-failure.json`. Wersja, schema 13 i 190 zewnętrznych
wersji bibliotek są rozliczane osobno: zmieniła się tylko wersja rGG na dev.31.

## Weryfikacja

- Wszystkie 124 testy Rust, w tym 44 przypadki korpusu i deterministyczne mutacje,
  przeszły. `cargo fmt --check`, Clippy wszystkich targets z `-D warnings` i build
  workspace także przeszły. Surowego pełnego stdout Rust nie archiwizowano;
  pomyślny wynik narzędzia opisano jawnie w raporcie regresji.
- Siedem zestawów TCP/HTTP przeszło w 26,949 s: sesje, obecność/kontakty,
  katalog, kontrakty sesji, statystyki, profil HTTPS i lifecycle.
  Nowa regresja opisów jest częścią zwykłej CI i suite kontenera.
- Niezmieniona libgadu 1.7.0: cztery połączenia z rzeczywistym LOGIN60/0x20,
  odrzucenie hasła, opis przy logowaniu i zmianie statusu, CP1250, niewidoczność,
  trzy wiadomości offline po awarii oraz dwie online w przeciwnych kierunkach.
  Potwierdzono bramkę Ready, FIFO, ACK, czyste SIGTERM oraz integralność bazy.
- Sprawdzono sumy 38 pierwotnych plików libgadu po kompilacji; żaden nie zmienił
  się. Własny klient zbudowano z ostrzeżeniami traktowanymi jako błędy. Builder
  i test działały w osobnych przestrzeniach sieciowych z samym loopback.
- Kontrola pochodzenia: 13 wpisów, 10 crate, 13 migracji. Plan pozostaje niezmienny.

Pierwsze dwie kontrole izolacji testera wymagały korekty: `/proc/1/ns/net` jest
niedostępny w user namespace, a współdzielony sysfs pokazuje zewnętrzne interfejsy.
Finalny sterownik porównuje namespace z wartością odczytaną przez launcher przed
`unshare` i używa `socket.if_nameindex()` dla faktycznie bieżącej przestrzeni.
To korekta sterownika; nie zmieniano konfiguracji sieci hosta.

## Artefakty

- Debug serwer: `f1176e600dbf8c9e64236341ca7d79014e181a0ec1acb8b1921af28aaafe670f`.
- Debug admin: `994b02b251bea7d208b04e061c6d9ae79cb076da44208d87f42ba7d5a7b77e2f`.
- Zewnętrzny tester: `9386791a4589d15a9d073ed151cc2c82a4a4a75dae4b67a4993d70d59250c8fd`.
- `libgadu-build-2026-11-02-dev31.json`: przypięte źródła, komendy, hashe i wersja gcc.
- `libgadu-client-2026-11-02-dev31.json`: pakiety, zdarzenia klienta i wynik.
- `libgadu-regressions-2026-11-02-dev31.json`: pełne wyniki siedmiu grup oraz sumy źródeł.

Poprzednie binarki zachowano w `/tmp/rgg-baseline-dev30`; zewnętrzna biblioteka,
jej źródła z COPYING i tester są w `/tmp/rgg-libgadu170-final`, poza repo/runtime.
Serwery, klienci i relay testów zakończone. Brak push, wdrożenia lub zmian GG.

To kwalifikacja wskazanych API libgadu, nie oryginalnej aplikacji GG6. Nadal
otwarte są jej inne przepływy, GUI GG6, długi soak i kwalifikacja wdrożeniowa.
Ostatni kwalifikowany obraz/R0 jest dev.30 i nie zawiera poprawki dev.31.
[Szczegóły semantyki i ograniczeń](LIBGADU-COMPATIBILITY.md).
