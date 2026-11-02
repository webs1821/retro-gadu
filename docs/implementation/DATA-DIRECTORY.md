# Prywatny katalog danych — dev.33

Na Unix rGG wymaga katalogu danych dostępnego wyłącznie dla właściciela.
Nowe katalogi tworzy z trybem0700. Jeżeli istniejący katalog daje jakiekolwiek
uprawnienia grupie lub innym użytkownikom, CLI i serwer odmawiają otwarcia,
zanim utworzą blokadę albo otworzą SQLite. Nie zmieniają uprawnień automatycznie.

To usuwa błąd odtworzony na dev.32: wskazanie istniejącego katalogu0755
pozwalało zainicjalizować bazę0644. Inny lokalny użytkownik mógł odczytać bazę,
choć plik kluczy0600 pozostawał niedostępny. Próba używała wyłącznie danych
syntetycznych i rzeczywistego odczytu jako UID65534. Problem nie wymagał
przełamania szyfrowania: baza zawiera również dane kont, kontakty i treści
wiadomości. Prywatny katalog kontenerowy0700 nie miał tej ekspozycji.

## Kontrakt otwarcia

- Katalog jest kanonikalizowany. Alias prowadzący do prywatnego katalogu
  pozostaje obsługiwany i korzysta z tej samej blokady systemowej.
- Istniejące `rgg.lock`, `rgg.sqlite`, `rgg.sqlite-wal`, `rgg.sqlite-shm`
  i `rgg.sqlite-journal` muszą być zwykłymi plikami. Dowiązania symboliczne,
  również wiszące, katalogi i pliki specjalne są odrzucane przed SQLite.
- Na Unix odrzucane są również twarde dowiązania tych plików. Blokada musi
  mieć uprawnienia tylko właściciela; sprawdzany jest także otwarty uchwyt.
- Program nie usuwa błędnego pliku, nie zmienia jego celu i nie naprawia
  uprawnień samoczynnie. Odmowa pozostawia dotychczasowe dane bez zmian.
- Ochrona prywatności bazy opiera się na katalogu. Starsze zwykłe pliki
  SQLite0644 pozostają akceptowane wewnątrz katalogu0700; nie należy ich
  przenosić do publicznego katalogu ani udostępniać osobno.

Administrator musi wskazać zaufaną ścieżkę, zabezpieczyć jej katalogi nadrzędne
i uruchamiać usługę jako przeznaczone do tego konto. Właściciel danych oraz
root są zaufani; kontrola nie obiecuje ochrony przed równoczesną podmianą
plików przez tego samego właściciela lub administratora systemowego.
Nie ma tu odczytu ACL Windows ani kwalifikacji natywnego uruchomienia Windows.
Do pracy w WSL należy używać systemu plików, który rzeczywiście egzekwuje
uprawnienia Unix, zamiast zakładać, że katalog dysku Windows ma ten sam model.

## Aktualizacja

Schemat pozostaje14. Poprawnie zabezpieczona istniejąca instalacja nie wymaga
migracji. Przy odmowie należy zatrzymać usługę, sprawdzić wskazaną ścieżkę,
własność i zawartość, a następnie świadomie skorygować uprawnienia katalogu
i blokady. Nie wykonywać rekursywnego chmod ani nie usuwać pliku blokady
działającego procesu. Kopie mają nadal osobne reguły weryfikacji i odtwarzania.

Przykład dla zweryfikowanego, własnego katalogu prywatnego po zatrzymaniu usługi:

```sh
chmod 700 /private/rgg-data
chmod 600 /private/rgg-data/rgg.lock
```

Druga komenda dotyczy wyłącznie istniejącego zwykłego pliku blokady, którego
własność i brak dowiązań zostały sprawdzone. Przy błędnym pliku lub nieoczekiwanym
właścicielu należy wyjaśnić przyczynę zamiast obchodzić kontrolę.

## Testy

Trzy nowe testy Rust obejmują istniejące katalogi0755/0750/0770/0777, odmowę
tworzenia blokady, dowiązania i uprawnienia oraz alias prywatnego katalogu.
`tests/data_directory.py` sprawdza dodatkowo25 przypadków dowiązań/FIFO/katalogów,
brak modyfikacji istniejącej bazy po odmowie, poprawne logowanie i wiadomości
po świadomej korekcie oraz wspólną blokadę serwera i CLI przez alias.
Fixture Rust tworzą teraz jawnie katalogi0700 zgodnie z kontraktem produkcyjnym.

[Raport dev.33](VERIFICATION-2026-11-02-dev33.md) zawiera dowód błędu dev.32,
wyniki regresji i ograniczenia kwalifikacji.
