# Rejestracja WWW — lokalny profil dev.7

Ustaw `web_registration = true` w głównej sekcji TOML. Potrzebny jest także
`gg_bind` i keyring przygotowany przez `rgg-admin init`; startup sprawdza klucze
przed otwarciem portów. Formularz jest pod `http://<http_bind>/register`.
Przy porcie 0 należy użyć rzeczywistego adresu listenera. Host musi odpowiadać
adresowi, np. `127.0.0.1:8080`; alias `localhost` nie jest równoważny.
Domyślnie rejestracja jest wyłączona (404). GET, POST i CSS pozostają lokalne.

## Model konta

Opcjonalny numer 1–16777215, pusty oznacza przydział od 112233. Hasło GG6 ma
8–15 drukowalnych znaków ASCII bez spacji. Kod zaproszenia przyjmuje istniejący
format 32 znaków hex; wielkość liter i otaczające spacje nie mają znaczenia.
E-mail jest walidowany, ale jeszcze nieweryfikowany. Nie służy do odzyskiwania.
Nie publikujemy profilu automatycznie. Nie ma sesji webowej ani samodzielnej
zmiany hasła przez ten formularz.

Adapter HTTP przekazuje polecenie do `RegistrationService`, który używa tego
samego portu kont co CLI. Zużycie kodu, przydział UIN i zapis szyfrowanego hasła
są jedną transakcją. Kolizja numeru lub błąd nie zużywają zaproszenia. Nowe
konto może zalogować się do GG6 bez restartu. Stare endpointy rejestracji nadal
odmawiają. Nie zmieniamy istniejących kont ani ich sesji.

## Granice i ochrona

- Host i Origin muszą być pojedyncze i równe skonfigurowanemu adresowi. POST
  wymaga Origin. Sec-Fetch-Site, jeżeli jest, dopuszcza same-origin lub none.
  Forwarded/X-Forwarded-* nie określają adresu ani tożsamości klienta.
- CSRF: losowe 256 bitów, cookie HttpOnly/SameSite=Strict, porównanie constant
  time, powiązanie z rzeczywistym IP, jednorazowość i 600 s ważności. Cookie
  ma Path=/register i nie ma Domain. Po użyciu token jest usunięty.
- Limit danych 4096 B; nieznane/powtórzone pola są odrzucane. Maksymalnie
  8 obsługiwanych żądań, deadline 5 s także dla wolnego body. GET/formularz/CSS
  60 na IP i 120 globalnie/min; POST 20 na IP i 60 globalnie/min.
- Budżety i tokeny używają zegara monotonicznego. Maksymalnie 2048 tokenów
  i 2048 rekordów IP; pojemność przewyższa dopuszczalne przyjęcia w 10 minut.
  Restart zeruje limity HTTP i tokeny; zużycie zaproszeń jest trwałe.
- no-store, nosniff, DENY, no-referrer, CSP ograniczona do własnego CSS/formularza,
  brak JavaScript i zewnętrznych zasobów. W HTML/logach brak odbijania danych
  formularza; sukces pokazuje tylko przydzielony numer.

Cookie nie ma Secure w lokalnym profilu HTTP. Publiczny TLS, reverse proxy,
rzeczywisty adres zaufanego proxy i trwałe limity nadużyć wymagają osobnej
kwalifikacji. Obecna konfiguracja odrzuca wszystkie bindy poza loopback.
Utrata połączenia po commit może ukryć sukces przed przeglądarką; nie obiecujemy
idempotentnego ponowienia rejestracji. Operator może sprawdzić stan offline CLI.

## Dowody

`tests/http_registration.py` uruchamia syntetyczne konta w usuwanych bazach:
logowanie i wiadomości, restart, dokładnie trzy rejestracje przy 12 konkurujących
żądaniach, brak wpisu publicznego, niezjedzone zaproszenie po błędzie, CSRF/replay,
złe Origin/Host/cookies, oversize, timeout i brak ominięcia limitu przez proxy
headers. Testy jednostkowe sprawdzają wygaśnięcie, powiązanie z IP i pojemność.
`tools/preview_registration.py --server ... --admin ...` daje osobny chwilowy
podgląd na danych testowych; Enter zamyka proces i usuwa jego dane.
