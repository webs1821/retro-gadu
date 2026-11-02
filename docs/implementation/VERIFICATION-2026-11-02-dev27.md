# Dev.27 — prywatne pomiary procesu

Lokalna implementacja CPU/RSS na Linux i jawnej dostępności oraz wieku próbek.
Schema pozostaje13. Sampler działa tylko z opcjonalnym listenerem metryk;
nie zmieniono danych użytkowników, protokołu GG ani reguł dostępu.

Przeszły fmt, Clippy wszystkich targetów/features,108 testów Rust i normalny build.
Siedem grup rzeczywistych testów HTTP/TCP zakończyło się poprawnie:

| Grupa | Czas |
|---|---:|
| Pomiary procesu, porównanie z jądrem, GG i restart | 1,506s |
| Prywatny endpoint, reguły dostępu i zamykanie gniazd | 7,176s |
| Liczniki runtime, histogramy i wolny odbiorca | 4,024s |
| Cykl życia, lock, restart i SIGTERM | 0,219s |
| 100000 wpisów historii,200 offline i retencja | 5,839s |
| Równoczesny replay:3000 historii i3000 offline | 34,076s |
| Krótki scenariusz R0 | 8,202s |

Surowe wyjście, czasy i sumy źródeł/binarek są w
process-metrics-2026-11-01-dev27.json (próba rozpoczęta przed północą).
Wszystkie sumy źródeł i sterowników sprawdzono po testach.

- Serwer debug:089c2d0346289e57407b109cd1bcc38ee4c0110348f62a7f6f00c17c9e346555.
- Admin debug:ccc8c5b6e1fa92e959fd84ee04246e442fd41892087bd546b0e96e89c2748022.
- Inwentarz190 zależności zachowuje te same wersje, licencje i checksumy.
- 11 wpisów pochodzenia,10 modułów Rust,13 migracji; plan pierwotny niezmieniony.
- Maksymalnie21 rodzin/117 szeregów, rozmiar poniżej16KiB; brak/starość nie udają zera.

Ostatni kwalifikowany obraz pozostaje dev.26. Dev.27 nie ma nowej kwalifikacji
obrazu,600-sekundowego R0 ani długiego soak. Nie wykonano push ani wdrożenia.
Opis kontraktu i ograniczeń: [PROCESS-METRICS.md](PROCESS-METRICS.md).
