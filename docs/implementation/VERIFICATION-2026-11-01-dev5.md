# Weryfikacja rGG 0.1.0-dev.5 — 2026-11-01

Lokalny Linux/WSL, Rust 1.99.1, wyłącznie syntetyczne konta i bazy tymczasowe.

| Kontrola | Wynik |
|---|---|
| Formatowanie / Clippy wszystkie cele i funkcje | poprawnie, bez ostrzeżeń |
| Testy Rust | 50 przeszło |
| Katalog GG6 TCP | 3 grupy przeszły |
| Obecność i archiwa TCP | 4 grupy przeszły |
| Logowanie, sesje i kolejka TCP | 5 grup przeszło |
| Cykl życia oraz konta CLI | oba zestawy przeszły, schemat 6 |
| Pochodzenie i granice modułów | 4 wpisy, 9 modułów, 6 migracji |
| Zależności / RustSec | 190 zewnętrznych wersji, 0 znanych podatności i ostrzeżeń |

Testy katalogu sprawdzają dobrowolność publikacji, właściciela i fencing starej
sesji, pełne zastępowanie i usuwanie formularza, odmowę przyszłego roku, polskie
filtry, dosłowne znaki SQL, mapowanie płci i strony 20/20/1. Sprawdzono natywne
kody odpowiedzi, zerową sekwencję READ/WRITE, niezerową SEARCH oraz brak pustego
dodatkowego wiersza przy wyszukiwaniu po roku. Profil przeżywa restart i znika
po usunięciu konta. Limit przeżywa zastąpienie sesji, restart i cofnięcie zegara.

Testy prywatności obejmują niewidoczność, blokady w obie strony, tylko znajomych
i ActiveOnly. Test bariery utrzymuje rozpoczętą publikację katalogu i potwierdza,
że usunięcie profilu czeka na jej zakończenie. Regresje wcześniejszych funkcji
przeszły na zwykłych binariach; nie dodawano nowych punktów wymuszania awarii.
Sześć istniejących scenariuszy awarii ma osobny wynik historyczny w raporcie dev.4.

Brak kwalifikacji natywnego GG i Windows. WWW, discovery, administrator online,
odzyskiwanie kont, migracja produkcji oraz dalsze funkcje pozostają do wykonania.
Nie uruchamiano usług produkcyjnych, push ani wdrożenia. Plan i recenzje pozostają
bez zmian. Szczegóły semantyki: [katalog publiczny](PUBLIC-DIRECTORY.md).

Nie zmieniono wersji zewnętrznych bibliotek. [Audyt](cargo-audit-2026-11-01-dev5.json)
i [inwentaryzacja](dependencies.json) zapisują dokładne dane.
SHA-256 Cargo.lock: `9808e16c8e86bf80c462b0fc125be7a3b1edb8dcbda0d4703fb0eb5f5dfbb82a`.
