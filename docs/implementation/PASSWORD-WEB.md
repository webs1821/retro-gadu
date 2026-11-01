# Zmiana hasła przez WWW — kontrakt dev.21

Implementacja i testy ukończone lokalnie, schema 12. Wyniki i granice kwalifikacji
opisuje [raport](VERIFICATION-2026-11-01-dev21.md). Nie podmieniano gg-retro.

## Zachowanie

Opcjonalne `web_password_change=true`, formularz `/password`; wymaga
zainicjalizowanego keyringu i runtime GG. Użytkownik podaje numer, pełne obecne
hasło oraz dwukrotnie nowe. Ta sama polityka 8–15 drukowalnych znaków ASCII bez
spacji. Brak resetu, poczty i włączenia dawnych endpointów zmieniających konto.

Pełne obecne hasło jest porównywane w stałym buforze, niezależnie od GG32.
Nieznany/wyłączony użytkownik, zły sekret i wyczerpany budżet dają ten sam
komunikat odmowy. Pięć prób na aktywne konto w oknie 600 s, również poprawne
próby; stan trwały, odporny na restart i cofnięcie zegara. Brak tworzenia
rekordów budżetu dla nieznanych numerów. Wspólny budżet IP/globalny WWW także
obowiązuje. Osobny cel CSRF i cookie ścieżki `/password`, jednorazowość,
powiązanie IP, Host/Origin, limit ciała i czas obsługi. Bez JS i echo sekretów.

## Serializacja i transakcja

Po weryfikacji powstaje niekonstruowalna z sieci komenda zawierająca tylko UIN,
oczekiwaną credential_version i zaszyfrowany nowy sekret. Aktor konta sprawdza
aktualność wersji przed zatrzymaniem klienta. Następnie bariera socketu i
wyłączna bramka publikacji poprzedzają transakcję: ponowne sprawdzenie wersji,
świeży nonce w historii, zmiana koperty i wersji, unieważnienie sesji/generacji,
zwolnienie lease do Pending, usunięcie obecności/staging i audyt. Kolejka,
archiwum, profil, prywatność oraz istniejące uprawnienia do obrazków pozostają.
Stare sesje nie mogą publikować; istniejące transfery obrazków sprawdzają
bieżącą sesję. Stare dowody logowania muszą być odrzucane przed zamknięciem
nowszego połączenia. Błąd przed commit pozostawia stare hasło.

TCP GG6 nie potwierdza odbioru: zwolnienie lease po zamknięciu klienta może
powtórzyć wiadomość zapisaną do TCP tuż przed zmianą. To ten sam jawny model
jak przy utracie połączenia. Utrata odpowiedzi HTTP może pozostawić zakończoną
zmianę; należy sprawdzić logowanie nowym hasłem, nie zakładać rollback.

Migracja 11→12 rozszerza actor w istniejącym audycie, zachowując rekordy
i licznik AUTOINCREMENT także przy pustej tabeli. CLI pokaże rzeczywistego
aktora. Limit 10000 rekordów i widok 30 dni wspólne dla CLI i WWW.

## Weryfikacja wykonana

- Błędne/niezmienione hasło i zły formularz bez zamknięcia sesji i mutacji danych.
- Bariera socketu i publikacji, stare logowanie przeciw nowszej sesji,
  konkurencyjne zmiany tego samego hasła oraz rollback nonce/version.
- Logowanie starym/nowym hasłem, reszta danych i Pending po zmianie/restart.
- Trwałe limity, nieznane UIN, cofnięty zegar, opt-in i CSRF między formularzami.
- Awaria przed/po commit, audyt bez sekretów, backup/restore i rzeczywista
  migracja z zachowanej binarki dev.20 w `/tmp/rgg-baseline-dev20`.
- Regresja CLI oraz współdzielonych protokołów/WWW; brak zmiany działającego GG.

## Konfiguracja i ograniczenia

```toml
http_bind = "127.0.0.1:8088"
gg_bind = "127.0.0.1:8074"
web_password_change = true
```

Opcja domyślnie wyłączona; błędna konfiguracja i `keys.pending` są odrzucane
przed otwarciem bazy. Formularz działa niezależnie od rejestracji i statystyk.
Link dodawany do włączonej strony głównej oraz katalogu pobierania; samo `/`
pozostaje wyłączone bez statystyk. Token ma własny cel w tym samym ograniczonym
magazynie co rejestracja (2048, 600 s). Cookie ma HttpOnly/SameSite=Strict
i Path=/password. Przy obecnym HTTP loopback nie ustawiamy Secure. Profil TLS,
proxy i publiczna domena nadal wymagają osobnej implementacji oraz kwalifikacji.

Ciało formularza do 4096 B, wspólnie do 8 żądań naraz, 20 POST/IP i 60 globalnie
na minutę; GET formularzy 60/IP i 120 globalnie. POST /password ma limit 15 s,
obejmujący oczekiwanie aktora (12 s), a bariera gniazda najwyżej 6 s. Pozostałe
formularze zachowują 5 s. Nie ma nowych zależności ani publicznych endpointów.

Błąd magazynu po zatrzymaniu gniazda może pozostawić stare hasło przy zamkniętej
sesji. Jest to świadoma kolejność: zapis następuje dopiero po barierze. Audyt
zawiera aktora, numer i wersje, bez hasła, GG32, nonce ani treści wiadomości.
Rejestr nonce i hasło zmieniają się w jednej transakcji z audytem. Odczyt CLI
pokazuje prawdziwe źródło zmiany (`local_cli_with_current_password` albo
`local_web_with_current_password`).

Kopie starego schematu odtwarzamy odpowiadającą im starszą binarką, następnie
uruchamiamy nową migrację na odtworzonym katalogu. Nie zmieniamy sum kontrolnych
ani manifestu starszego archiwum. Sprawdzono to dla zapisanej dev.20/schema11.
