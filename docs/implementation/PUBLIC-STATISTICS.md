# Publiczne statystyki — dev.8

`web_public_stats = true` w głównej sekcji konfiguracji włącza lokalne `/`
oraz `/api/stats`. Domyślnie obie ścieżki są wyłączone. Opcja jest niezależna
od `web_registration`; formularz i link do niego pojawiają się tylko, gdy
rejestracja jest włączona. Bez portu GG statystyki nadal mogą pokazać istniejące
konta; aktywnej obecności wtedy nie ma. Profil bindu pozostaje loopback.

JSON sukcesu ma wyłącznie `registered`, `publicly_online` oraz `sampled_at`
(sekundy Unix, UTC). Strona HTML pokazuje te same dwa liczniki. Nie są to
metryki sesji operatora ani historyczna liczba rejestracji:

- registered: wszystkie istniejące konta, także wyłączone; bez trwałych rezerwacji
  numerów po usunięciu. Liczymy konta, nie unikalne osoby.
- publicly_online: unikalne aktywne konta z gotową sesją bieżącej epoki,
  zgodnymi generation/credential_version/session_id oraz obecnością dostępną
  anonimowemu obserwatorowi. Dostępny/zajęty są online; offline, niewidoczny
  i tylko znajomi nie są. Lista indywidualnych blokad nie ujawnia się w agregacie.
- Brak profilu publicznego nie zmienia jawnego statusu konta. Opis, czas powrotu,
  adresy IP, UIN, e-mail, sekret i lista uczestników nigdy nie wychodzą w odpowiedzi.

Repozytorium odczytuje oba liczniki w jednej transakcji odczytu SQLite.
Strumieniuje wyłącznie rodzaj statusu i flagę tylko znajomi aktualnych sesji;
używa `presence::publicly_online`, opartego na wspólnej polityce widoczności.
Nie ma drugiej, uproszczonej flagi is_online ani cache opartego na połączeniach.
Brak wymaganych rekordów/zgodności sesji wyklucza konto z online.

Wynik jest migawką z chwili obsługi żądania, a nie subskrypcją. Równoczesna zmiana
statusu może nastąpić po jej odczycie; odpowiedź nie trzyma bramki publikacji
przez późniejszy zapis HTTP. To anonimowy agregat z `sampled_at`, nie lista
statusów użytkowników. Strona wymaga odświeżenia, aby zobaczyć nowy stan.
Brak odpowiedzi z bazy daje błąd, nigdy pozornie poprawne zera.

Wspólny adapter strony (`website.rs`) stosuje te same kontrole Host, pochodzenia,
gotowości, deadline, współbieżności i IP oraz nagłówki co rejestracja. GET strony,
API, CSS i formularza współdzielą 60 żądań/IP/min i 120 globalnie/min. Odpowiedzi
nie są cache'owane; Forwarded/X-Forwarded-* nie zmieniają tożsamości ani adresu.
HTTP discovery i health zachowują osobne kontrakty. Publiczne TLS/proxy, wydajność
dużej bazy i kwalifikacja natywnego klienta pozostają poza tym przyrostem.

Dowody: `statistics_tests.rs` i `tests/http_statistics.py` oraz
[raport dev.8](VERIFICATION-2026-11-01-dev8.md). Nie zmieniono schematu bazy.
