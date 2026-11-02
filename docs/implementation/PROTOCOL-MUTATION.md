# Deterministyczne testy mutacyjne parserów GG6

Dodano do dev.27 jako osobny krok walidacji, bez zmiany runtime, schematu lub
zależności. Test integracyjny `crates/rgg-protocol/tests/adversarial.rs` działa
automatycznie w `cargo test --workspace`; nie korzysta z sieci, dysku danych,
kont użytkowników ani natywnego klienta.

## Korpus i kontrakty

41 własnych syntetycznych przypadków w `tests/fixtures/wire-corpus.json`
obejmuje LOGIN60, statusy, fragmenty NOTIFY, archiwa kontaktów, katalog,
wiadomości ze stylami/konferencją/obrazkiem oraz kontrolę obrazków. Każdy
ma oczekiwane przyjęcie lub odrzucenie. Korpus zawiera polskie CP1250,
powielone klucze i aliasy, błędne numery, brak terminatora i złe długości.
Poprawne granice obejmują70 znaków opisu,400 kontaktów,2047 bajtów archiwum,
2000 znaków wiadomości,256 stylów oraz1900 bajtów fragmentu obrazka.

Dla każdego przypadku sprawdzane są wszystkie prefiksy i wszystkie zmiany
pojedynczego bitu. Skrócenie może być innym poprawnym pakietem, np. krótszym PUT;
nie zakłada się automatycznie błędu każdego prefiksu. Poprawne wyniki muszą
zachować limity, numery, bajty tekstu po konwersji CP1250 i możliwość utworzenia
ograniczonej odpowiedzi. Obowiązujące zasady prywatności i sesji nadal sprawdza
warstwa aplikacji, a nie sam parser LOGIN60.

Ramki są budowane niezależnie od produkcyjnego serializera. Sprawdzane są
wszystkie podziały nagłówka/payloadu, sklejone ramki, zmiany pól nagłówka,
zerowa długość,65535/65536/65537 i u32::MAX. Dekoder musi pobrać dokładnie
pełną ramkę albo pozostawić bufor bez zmian. Błąd długości nie czeka na payload.

## Powtarzalne kampanie

Zwykły test dodaje20000 mutacji ze stałym seedem. Własny generator testowy
wykonuje1–4 operacje: bit, zastąpienie bajtu/liczby, skrócenie, dodanie/usunięcie,
duplikacja fragmentu. Co szesnasty przypadek zaczyna od losowych bajtów.
Wejście ograniczono do8192 bajtów; seed musi być niezerowy, liczba iteracji
mieści się w1..2000000. Generator nie służy kryptografii.

Przykład w środowisku Rust/WSL:

```sh
RGG_PROTOCOL_SEED=1 RGG_PROTOCOL_CASES=250000 cargo test -p rgg-protocol --test adversarial deterministic_mutation_campaign -- --exact --nocapture
```

Panic/assert wypisuje target, seed/iterację lub nazwę przypadku oraz dokładne
syntetyczne bajty, po czym nadal kończy test niepowodzeniem. Raport podaje także
liczby przyjęć i odmów każdego targetu. Nowe znalezione błędy należy minimalizować
i zachować jako przypadki korpusu regresji.

Rozszerzona próba z2026-11-02 ma osobny plan i raport `protocol-mutation-*-final*`:
osiem seedów po250000 mutacji, czyli2000000 w rozszerzonych kampaniach;
dodatkowo uruchamiane są domyślne testy korpusu i workspace. Timeout każdej
kampanii wynosi60s. Plan zapisano przed wykonaniem i zawiera sumy źródeł.
Próba zakończyła się poprawnie:41 wzorców,11545 prefiksów,92032 zmian jednego
bitu oraz wszystkie2000000 rozszerzonych mutacji. Osiem kampanii zajęłołącznie
16,429s (osobne testy pełnego korpusu trwały20,805s). Przeszły także fmt,
Clippy i111 testów Rust. Plan, hash binarki testowej, wyniki i sumy źródeł
zapisano w raporcie; sprawdzono je po zakończeniu.
Pierwszy preflight bez końcówki final zatrzymał Clippy na dwóch użyciach
chunks_exact; zastąpiono je as_chunks i zachowano jego raport, bez nadpisania.

## Granice dowodu

To deterministyczne testy mutacyjne bez informacji o pokryciu. Nie zastępują
fuzzingu sterowanego pokryciem, sanitizerów, sekwencji komend w złych stanach
sesji, dekompresji instalatora ani testów GG na rzeczywistym kliencie.
Brak panic w skończonym korpusie nie oznacza dowodu bezpieczeństwa wszystkich
wejść. Przypadki są własne, oparte o fakty `libgadu-wire-reference`
z THIRD_PARTY.toml; nie skopiowano kodu ani korpusu innego projektu.
