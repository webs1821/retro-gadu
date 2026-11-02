# CPU i pamięć własnego procesu — dev.27

Opcjonalny prywatny listener metryk uruchamia jeden sampler co sekundę.
Wyłączone metryki nie uruchamiają samplera. Odczyty systemowe odbywają się
przed krótkim zamkiem telemetry, niezależnie od żądań HTTP. Pominięte takty
nie tworzą kolejki zaległych odczytów. Wspólny shutdown anuluje i dołącza zadanie.

| Rodzina | Typ i znaczenie |
|---|---|
| `rgg_process_cpu_seconds_total` | Counter: czas CPU wszystkich wątków procesu, bez procesów potomnych |
| `rgg_process_resident_memory_bytes` | Gauge: przybliżone RSS własnego procesu w bajtach |
| `rgg_process_measurements_available{resource="cpu"\|"resident_memory"}` | Dwa stałe gauge 0/1: źródło dostępne i próbka nie starsza niż 5 s |
| `rgg_process_sample_age_seconds` | Monotoniczny wiek ostatniej próby, liczony od początku odczytów |

CPU mierzy bezpieczne API `rustix::time::clock_gettime_dynamic` z
`DynamicClockId::Known(ClockId::ProcessCPUTime)`. Używamy wariantu zwracającego
Result; niepowodzenie, ujemna liczba lub nieprawidłowe nanosekundy oznaczają brak
pomiaru. Rustix 1.1.6 był już w lockfile: dodano bezpośrednią zależność Linux
z feature time, bez zmiany żadnej ze 190 wersji bibliotek i bez własnego unsafe.

RSS pochodzi wyłącznie z `/proc/self/status`, pola VmRSS. Odczyt ograniczono do
16 KiB + jednego bajtu kontrolnego; większy dokument jest odrzucany. Parser
obsługuje bajty, więc nie wymaga UTF-8 w nazwie procesu. Kontroluje jednostkę kB,
liczbę całkowitą, mnożenie przez 1024 i powielone pola. Nie czyta innych PID,
smaps ani dowolnie konfigurowanej ścieżki. Odczyt procfs jest synchroniczny,
krótki i ograniczony rozmiarem; nie ma osobnej gwarancji czasu dla uszkodzonego
lub zastąpionego procfs. Brak podprocesów i puli kolejkowanych zadań pomiarowych.

W razie braku źródła eksportujemy availability=0 i pomijamy jego wartość,
zamiast udawać zerowe użycie. Próbka starsza niż 5 s nie publikuje CPU/RSS,
ale zachowuje wiek. Przed pierwszą próbą brak także wieku. Na innych systemach
niż Linux wartości CPU/RSS są jawnie niedostępne. Rzeczywisty odczyt zero jest
wartością poprawną i różni się od braku odczytu.

Cały prywatny endpoint ma 21 rodzin i maksymalnie 117 szeregów, nadal poniżej
16 KiB. Nie dodano PID, ścieżek ani danych kont do etykiet. Nie zmieniono
reguł dostępu, cache/deadline bazy ani publicznego `/api/stats`.

Do udziału CPU w czasie służy pochodna licznika, np.
`100 * rate(rgg_process_cpu_seconds_total[5m])`: 100 oznacza jeden rdzeń,
a proces wielowątkowy może przekroczyć 100. Licznik zaczyna od nowa z procesem.
Nie należy dzielić go przez uptime listenera ani utożsamiać RSS z pamięcią
kontenera, sterty lub całego hosta. RSS jest przybliżeniem jądra.
Prometheus nie został instalowany ani uruchamiany.

## Weryfikacja i pochodzenie

Własne testy sprawdzają granice parsera i faktyczny limit czytanych bajtów,
błędy I/O, nie-UTF8, duplikaty i overflow; brak/starość/częściowe próbki,
maksymalne liczby i rozmiar odpowiedzi, monotoniczny CPU oraz zakończenie zadania.
Test `process_metrics.py` porównuje wartości HTTP z niezależnym `/proc/PID/stat`
i `/proc/PID/status`, sprawdza opt-in, działający GG, restart i czysty SIGTERM.

Implementacja własna, bez portowania kodu dokumentacji. Referencja
`process-observations-reference` w THIRD_PARTY.toml:
[Linux procfs](https://www.kernel.org/doc/html/latest/filesystems/proc.html)
oraz [rustix time API](https://docs.rs/rustix/1.1.6/rustix/time/index.html).
Dokładne API sprawdzono w lokalnym źródle przypiętej paczki rustix 1.1.6
(`src/time/clock.rs`), a RSS w dokumentacji jądra.

Wynik obrazu/R0 dev.26 nie kwalifikuje nowego kodu dev.27. Nadal brak długiego
soak, kwalifikacji natywnego klienta i publicznego wdrożenia.
