# Weryfikacja dev.20 — 2026-11-01

Zakres: opcjonalny formularz przygotowania GG6 w przeglądarce, własny moduł ES,
recepta JSON, generator mapy, integracja HTTP i konfiguracji. Schemat 11,
10 modułów Rust i 190 zewnętrznych wersji bibliotek pozostają bez zmian.
Wszystko lokalne, na sztucznych kontach; bez push i wdrożenia.

## Wykonane kontrole

- Rust 1.99.1: build, fmt, **89 testów**, Clippy workspace/all-targets/all-features
  z `-D warnings`. Normalne binarki bez test-hooks.
- Node **24.11.1**, Python **3.13**: kontrola składni obu modułów ES i 6 pełnych
  wektorów zgodności z Pythonem: syntetyczny PE i rzeczywisty, przypięty GG6,
  po trzy adresy (`127.0.0.1`, `rgg.local`, 16 znaków).
- Uszkodzony plik, długość, adresy, zakresy, overlap, błędne warunki stałych
  poprawek i prefiksy RT_STRING odrzucone. Wejście pozostaje niezmienione;
  test zmiany wejścia/mapy podczas digest potwierdza użycie kopii.
- Prawdziwe HTTP: `http_patcher.py` — disabled/opt-in, exact assets, HEAD,
  CSP/MIME/no-store, brak uploadu, Host/Origin/fetch-site, query/body,
  współdzielony budżet i odrzucenie złych ustawień przed utworzeniem danych.
- Regresje: pobieranie, statystyki, rejestracja/CSRF, metryki i lifecycle.
- Przeglądarka desktop IAB: nieprawidłowy plik odrzucony, poprawny plik
  przygotowany i pobrany; brak błędów/ostrzeżeń JavaScript. Po zmianie adresu
  stary link znika, ponowne przygotowanie daje nowy hash. Układ sprawdzony
  wizualnie. Nie jest to pełna macierz przeglądarek ani test natywnego GG.

Pobrany `E:\Downloads\gg.exe` jest **kopią testową dla 127.0.0.1**.
Nie uruchomiono go i nie podmieniono zainstalowanego klienta. Jego hash
sprawdzono również z dysku. Podgląd HTTP zatrzymano po teście; karta zamknięta.
Zrzut: `D:\Dokumenty\Documents\ChatGPT\Serwery\rgg-browser-local-2026-11-01\patcher-success.jpg`.

## Sumy SHA-256

| Artefakt | SHA-256 |
|---|---|
| Debug rgg-server dev.20 | `1eccd3201968cc4244f8e72c104a7b3322f17d632a80aa3fca6778551e6704d1` |
| Mapa zmian, bieżące bajty JSON | `3d0d0f6cd9261c58cc3927170daefe00c7d58d0520dc9988cf909fe3857a7942` |
| Wyjście dla 127.0.0.1 | `ddbc24fc91ffbb5e4bd29ff628436e60a8eb1ca4a7cd617bad3e32910bdf541f` |
| Wyjście dla rgg.local | `6cd324c011fce3e3df7f35e2689892fe0dfacd0e0c74032b529ef8376a631031` |

Plan implementacji pozostaje bez zmian: SHA-256
`cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec`.
Nie budowano nowego kontenera i nie powtarzano R0. Zapisany, sprawdzony
obraz **dev.19** pozostaje osobnym artefaktem; jego pomiary nie są wynikami
dev.20. Nie dodano zewnętrznych zależności ani nie ponawiano bez potrzeby audytu
tego samego zestawu bibliotek (ostatni zapis dev.18: 0 podatności/0 ostrzeżeń).
