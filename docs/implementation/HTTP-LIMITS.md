# Ograniczenia transportu HTTP — dev.18

Przed parsowaniem HTTP nowy listener egzekwuje limit przyjętych połączeń i
połączeń na rzeczywisty adres IP TCP. Domyślnie jest to 64 globalnie i 16 na IP.
Nadmiarowe gniazdo zostaje zamknięte, bez odpowiedzi HTTP i bez tworzenia zadania
obsługi żądania. `X-Forwarded-For` nie zmienia tej decyzji. Licznik i wpis IP
są zwalniane wraz z gniazdem; mapa nie przechowuje historii adresów.

```toml
[http]
max_connections = 64
per_ip = 16
idle_seconds = 15
write_seconds = 10
lifetime_seconds = 600
```

Wartości są opcjonalne (jak wyżej), a nieznane pola są błędem. Zakresy:
połączenia 1–1024, na IP 1–limit globalny, idle 1–300 s, write 1–120 s,
lifetime 2–3600 s i co najmniej tyle, ile idle oraz write. Limity nie zastępują
dotychczasowych ograniczeń żądań, CSRF, rozmiaru body, czterech pobrań i budżetu IP.

## Znaczenie czasu

- Idle oznacza brak rzeczywistego postępu odczytu **i** zapisu. Odbiorca dużego
  pliku nie musi przesyłać dodatkowych danych, aby utrzymać aktywne pobieranie.
- Write zaczyna się przy pierwszym zablokowanym zapisie TCP. Nowe dane od
  klienta oraz pusty flush nie przedłużają go. Dodatni zapis bajtów kończy
  ten okres; kolejna blokada otrzymuje nowy budżet.
- Lifetime jest nieprzedłużalny, od przyjęcia gniazda. Obejmuje keep-alive,
  częściowe nagłówki i odpowiedzi. Może przerwać bardzo długi, nadal aktywny
  transfer. Pobieranie nie obsługuje Range, więc w takim przypadku zaczyna się od nowa.

Po upływie terminu I/O zwraca błąd, połączenie jest zamykane, a zajęty slot pliku
wraca do puli. Są osobne rejestracje timerów odczytu i zapisu: równoczesny odczyt
nie może przejąć wybudzenia oczekującego zapisu. Kod nie używa unsafe ani nowej
biblioteki. Adapter axum zachowuje ConnectInfo z faktycznym adresem peer-a.

Granice dotyczą gniazd przyjętych do obsługi aplikacji. Kernel ma osobne kolejki
TCP; nie jest to ochrona przed wolumetrycznym atakiem sieciowym. Nie rejestruje
się każdej odmowy, adresów IP ani fragmentów nagłówków. GG ma własny listener,
budżety i terminy; zajęcie HTTP nie zużywa limitu GG. Przy pełnej puli HTTP
również health nie dostaje osobnego slotu — sprawdza się dostępność GG i odzyskanie
HTTP po zamknięciu/timeout, nie gwarancję health podczas pełnego nasycenia.

## Sprawdzenie

Testy Rust z kontrolowanym czasem obejmują postęp w obu kierunkach, bezwzględny
czas życia, zablokowany zapis mimo odbieranych bajtów, flush oraz globalne/per-IP
limity i sprzątanie adresów. Test prawdziwych gniazd:

```sh
python tests/http_limits.py --server target/debug/rgg-server --admin target/debug/rgg-admin
```

Używa wyłącznie loopback, sztucznej bazy i sztucznego pliku 16 MiB. Sprawdza
niekompletne nagłówki, podawanie bajtów co 0,2 s, niezależne źródłowe adresy
loopback, cztery nieczytające klienty pobierania, odzyskanie slotów bez zamknięcia
klientów oraz logowanie/ping GG. Testy wszystkich dotychczasowych funkcji HTTP
są regresją zmian listenera. Nie jest to długotrwały soak ani kwalifikacja
publicznego reverse proxy; nasłuch nadal dopuszcza tylko loopback.
