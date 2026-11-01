# Walidacja funkcji wysyłania kontaktów GG6.1

Samodzielny program MIT, uruchamiany jako osobny proces. Nie importuje kodu
serwera ani patchera rGG. Korzysta z zewnętrznego Unicorn2.1.4; biblioteka
Unicorn oraz oryginalny klient nie są dołączane do repo. Ich warunki licencji
pozostają odrębne. Recepta patchera rGG zachowuje licencję AGPL projektu.

Na Windows x64 zależność można przygotować w osobnym katalogu:

```powershell
python -m pip --isolated install --index-url https://pypi.org/simple --no-deps --only-binary=:all: --require-hashes --target D:\rgg-validation-deps -r validation/gg6_upload_cpu/requirements-windows.txt
python validation/gg6_upload_cpu/check.py --original D:\original.bin --patched D:\patched.bin --dependency-dir D:\rgg-validation-deps --report D:\new-cpu-report.json
```

Program sprawdza hash oryginalnego GG6.1.0.158. Do pamięci emulatora trafia tylko
funkcja uploadu i 128B miejsca poprawki. Wywołania strlen, memcpy oraz nadania
pakietu są przechwytywane. Nie ma loadera Windows, API systemu ani sieci.
Każda próba ma limit instrukcji/czasu. Sprawdzane są stos, rejestry zachowywane
przez wywoływaną funkcję, wynik EAX, licznik oczekiwanych odpowiedzi oraz brak
zmian danych wejściowych i pozostałych pól obiektu.

79 przypadków obejmuje wszystkie wielokrotności2047 do limitu klienta50000,
sąsiednie długości, pustą listę, NULL i dane przekraczające limit. Dokładne
pakiety ośmiu przypadków trafiają do raportu jako syntetyczne sekwencje.
Osobny test tests/client_upload_wire.py może przesłać te pakiety przez TCP
do lokalnego serwera i sprawdzić publikację całej listy oraz restart.

To sprawdzenie wybranej funkcji procesora i interfejsu pakietowego.
Cała aplikacja GG6, jej interfejs i rzeczywisty stos sieciowy pozostają
osobnym wymaganiem kwalifikacji.
