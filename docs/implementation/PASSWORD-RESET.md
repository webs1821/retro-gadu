# Lokalny reset zapomnianego hasła — dev.32

Administrator rGG może ustawić nowe hasło bez znajomości poprzedniego. Operacja
wymaga zatrzymanego serwera, wyłącznej blokady katalogu danych, oryginalnych
kluczy szyfrowania i utworzenia pełnej kopii przed zmianą. Uprawnienie wynika
z dostępu do tych plików; nie jest to odzyskiwanie konta przez formularz WWW.

```sh
python tools/reset_password.py --admin target/debug/rgg-admin \
  --data-dir /private/rgg-data --uin 12345 \
  --backup /private/backups/before-reset-12345
target/debug/rgg-admin password-audit --data-dir /private/rgg-data
```

Katalog nadrzędny kopii musi istnieć, a wskazany katalog kopii musi być nowy.
Pomocnik wymaga interaktywnego terminala, ponownego wpisania numeru konta
i dwukrotnego, ukrytego wpisania nowego hasła. Polityka GG6 to 8–15 drukowalnych
znaków ASCII bez spacji. Hasło trafia wyłącznie na stdin procesu potomnego.
Obiekty sekretów w Rust są zeroizowane; Python nie gwarantuje wymazania swoich
obiektów z pamięci. Narzędzie nie wyświetla ani nie eksportuje poprzedniego hasła.

Bezpośrednie `password-reset --data-dir KATALOG --uin NUMER --backup NOWY_KATALOG`
przyjmuje do 4096 B JSON na stdin, z dokładnie polami `confirm_uin` (liczba)
i `new_password` (tekst). Numer potwierdzenia musi odpowiadać argumentowi.
Nie ma opcji przekazania hasła w argumentach. Błędy parsowania nie zawierają
przesłanych danych.

## Kolejność i atomowość

1. CLI sprawdza komplet opcji, istniejącą bazę i oryginalny keyring; następnie
   uzyskuje blokadę katalogu. Działający serwer uniemożliwia reset.
2. Sprawdza brak niedokończonej rotacji `keys.pending`, potwierdzenie numeru
   i politykę hasła. Ładuje istniejące klucze, bez ich regenerowania.
3. Tworzy kompletną kopię: baza, klucze, kontrola wszystkich kopert sekretów,
   hashe i manifest publikowany na końcu. Szczegóły: [BACKUPS.md](BACKUPS.md).
4. W jednej transakcji sprawdza epokę procesu, aktywność konta, brak jego sesji,
   uwierzytelnienie starej koperty AEAD i różnicę nowego hasła. Zwiększa wersję
   poświadczeń oraz generację sesji z kontrolą przepełnienia, rezerwuje świeży
   nonce, zapisuje szyfrogram i audyt, usuwa limit prób zmiany hasła tego konta.

W razie błędu kopii transakcja resetu nie zaczyna się. W razie błędu transakcji
nie zostaje częściowa zmiana hasła, wersji, generacji, nonce, limitu ani audytu.
Nieistniejące lub usunięte konto oraz niezmienione hasło powodują odmowę.
Pełna kopia utworzona przed taką odmową pozostaje dostępna; CLI jej nie usuwa.
Po przerwaniu przed publikacją manifestu niepełna kopia nie przechodzi weryfikacji.
Do ponowienia należy użyć nowego katalogu, zachowując wcześniejsze pliki.

Numer, adres e-mail, profil, reguły prywatności, kontakty, zaproszenia, wiadomości
i uprawnienia do obrazków pozostają zachowane. Reset nie aktywuje usuniętego
konta i nie przyznaje mu dostępu do nowego konta o tym samym numerze.
Stara wersja poświadczeń nie może otworzyć sesji po zatwierdzeniu resetu.

## Audyt, migracja i odtworzenie

Schemat 14 dodaje parę `actor=local_cli_recovery`, `action=password_reset`.
Zmiany wymagające starego hasła nadal mają `action=password_changed` i odrębne
kanały CLI/WWW. Ograniczenie SQL nie pozwala pomylić tych par. Kanał operacji
nie jest poświadczoną tożsamością konkretnego administratora systemu.
Retencja pozostaje taka sama: do 10000 wpisów i 30 dni, odczyt ostatnich 100;
audit zapisuje zatwierdzone operacje, bez sekretów i bez odrzuconych żądań.

Migracja zachowuje istniejące wpisy oraz licznik AUTOINCREMENT, także jeżeli
retencja usunęła wszystkie wpisy. CLI otwiera i migruje bazę przed utworzeniem
kopii resetu: ta kopia ma schemat 14 i poprzednie hasło. Kopię sprzed aktualizacji
programu należy przygotować oddzielnie starą wersją narzędzia.

Odtworzenie kopii sprzed resetu przywraca stare hasło, a kopii po resecie — nowe.
Przy niepewnym wyniku po przerwaniu sprawdź audyt oraz logowanie przed ponowieniem.
Kopię schematu 13 odtwarza dev.31 do nowego katalogu, potem dev.32 migruje bazę.
Nie należy otwierać schematu 14 starszym programem ani usuwać migracji ręcznie.

## Weryfikacja

Testy Rust pokrywają szyfrowanie, kolizję nonce, błędne klucze, wycofanie generacji,
sesję aktywną, stare poświadczenia i epokę, konto nieaktywne/brakujące,
przepełnienia i dozwolone pary audytu. `tests/password_reset_cli.py` sprawdza
obowiązkową kopię, zachowanie danych, rzeczywiste logowania TCP GG6, przywrócenie
obu kopii, odmowy oraz migrację bazy utworzonej przez rzeczywisty dev.31.
Opcja `--faults` wymaga buildu z `rgg-admin/test-hooks` i przerywa proces
przed publikacją kopii oraz przed i po commit resetu. Zestaw jest w lokalnym
scenariuszu kontenerowym i CI. Wyniki: [raport dev.32](VERIFICATION-2026-11-02-dev32.md).

Nie zmieniano działającego gg-retro ani kont użytkowników. Wszystkie opisane
próby dotyczą nowego rGG, lokalnych procesów i syntetycznych danych.
