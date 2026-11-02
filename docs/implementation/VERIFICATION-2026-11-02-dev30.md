# Weryfikacja dev.30 — profil HTTPS za lokalnym proxy

Linux/WSL, syntetyczne konta i certyfikaty, wyłącznie loopback. Schemat 13
i 190 wersji bibliotek bez zmian. Profil WWW jest opcjonalny i nie znosi
ograniczenia bindów do loopback.

- 123 testy Rust workspace, formatowanie, Clippy all-targets/all-features
  z `-D warnings` oraz normalny build przeszły.
- 12 grup procesowych/TCP/HTTP przeszło: http_proxy, http_registration,
  http_password, http_statistics, http_downloads, http_patcher, http_limits,
  http_metrics, http_discovery, discovery_proxy, gg6_sessions i lifecycle.
- Cały zapisany przebieg wraz z Rust/build trwał 74,736 s. Test proxy użył
  rzeczywistego TLS1.3, weryfikacji nazwy rgg.test i jednorazowego certyfikatu.
  Próby obcej nazwy i niezaufanego certyfikatu zostały odrzucone.
- Rejestracja i zmiana hasła zachowują IP, cel i jednorazowość CSRF, Secure
  przy wydawaniu i kasowaniu ciasteczek, sesję GG i nowy login. Token z IP A
  nie działa z IP B, ale nadal działa z A. Stara sesja kończy się po zmianie.
- Pobieranie ma zgodne bajty i nagłówki; zasoby patchera są identyczne ze
  źródłami, CSP pozostaje właściwa. Metryki, health i discovery zachowują
  odrębne reguły; domena WWW nie nadaje dostępu do prywatnych metryk.
- Złe/duplikowane/brakujące nagłówki i obcy peer są odrzucane. Mapowany IPv6
  i IPv4 współdzielą limit; podszyte nagłówki przy rzeczywistym proxy nie
  dzielą budżetu POST, inny klient nadal ma swój limit. Brak mutacji kont.
- 13 błędnych konfiguracji odrzucono przed utworzeniem bazy. Testy Rust
  dodatkowo sprawdzają kanoniczne origin, aliasy i porty, brak ścieżek,
  publicznego proxy i publicznych bindów.
- Kontrola repo: 12 wpisów pochodzenia, 10 crate, 13 migracji. Inwentarz,
  niezmieniony SHA pierwotnego planu, 2 testy container_context i diff-check
  przeszły. Zweryfikowano sumy wszystkich źródeł zapisanych w raporcie.

[Raport z pełnymi wynikami i hashami](web-proxy-2026-11-02-dev30.json).
Normalne binarki debug:

- rgg-server: `daf5b49fec22be520fa68d66a39ffc7b88bc4f01482744158c350d4531b84b3b`
- rgg-admin: `3e4fcab3667ee82386f390240da8bd946fe73623c8673eee15f285ce698292b7`

W pierwszej próbie przygotowania nowego testu zaproszenie tworzono CLI po
uruchomieniu serwera. Poprawna blokada katalogu odmówiła dostępu. Przygotowanie
przeniesiono przed start serwera; nie zmieniano blokady ani reguł runtime.
Końcowy raport powyżej dotyczy pełnego przechodzącego przebiegu.

Nowy test dodano do CI i przyszłego container_suite (32 grupy). Etap obrazu
verification sprawdza obecność CLI OpenSSL potrzebnego tylko do testu TLS;
etap runtime nie instaluje nowego narzędzia. Nie wykonano nowego obrazu/R0,
instancji nginx, natywnego klienta, długiego soak ani publicznej kwalifikacji.
Ostatni kwalifikowany obraz pozostaje dev.26. Brak push/deploy, zmian DNS,
systemowych certyfikatów, zainstalowanego GG i działającego gg-retro.
Procesy, wątki proxy i katalogi tymczasowe zostały zakończone/usunięte.
