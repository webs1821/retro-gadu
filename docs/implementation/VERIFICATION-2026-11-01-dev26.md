# Dev.26 — pełna historia i migracja 13

[Opis poprawki i granice testów](HISTORY-RETENTION.md).

- 103 testy Rust, fmt i Clippy all-targets/all-features z -D warnings przeszły.
- 25 grup TCP/HTTP/CLI/migracji: history-regression-2026-11-01-dev26.json.
- 9 grup kontrolowanych awarii: history-faults-2026-11-01-dev26.json;
  restored_normal=true, porównane hashe zwykłego serwera/admina.
- Odtworzenie kopii utworzonej przez dev25 i migracja 12→13: zachowano
  12 sum, konta, zaproszenia, prywatność, wiadomości, obrazki, kontakty i profil.
  Nowy zapis oraz nowy backup/restore przeszły; źródło i stara kopia niezmienione.
- Formalny test 100000 historii + 200 offline/20 odbiorców: replay 3.020 s,
  ACK nowego przyjęcia 12.959 ms, restart i dokładna retencja przeszły.
  To WSL bez cgroup, nie próbka R0. Nie dziedziczy wyników obrazu dev24.
- Wcześniejsza nieudana diagnostyka dev25 i udana próba dev26 zachowane osobno,
  bez zastępowania nieudanego raportu udanym.
- Inwentarz 190 bibliotek, 10 modułów i 10 wpisów pochodzenia. W lockfile
  zmieniono tylko wersje modułów workspace; brak nowych bibliotek.
- Plan niezmieniony:
  cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec.

Zwykły serwer SHA256:
63f80a1c657000fb07d5b8a893eae6902a900cce8e4930c802d96fe3955f6b73.
Admin SHA256:
b00fb3f370eb2a24ac8be6e1dfbed116bef3b2a7e21b8b5567e1cdfeeca9845e.

Nie zmieniono produkcji, zainstalowanego GG, plików pobierania na stronie ani
DNS/hosts. Nie wykonano push. Pełna aplikacja GG6 pozostaje niekwalifikowana.
