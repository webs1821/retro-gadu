# Punkt wznowienia — 2026-11-01, dev.26

## Zakres

Użytkownik wznowił ciągły rozwój lokalny do przerwania przez niego. Nie pushować,
nie wdrażać na Containers, nie zmieniać działającego gg-retro, zainstalowanego GG,
DNS ani hosts. Hasło własnego konta użytkownika odzyskano wcześniej: nie powtarzać
odczytu i nie zapisywać go w repo. Bez aktywnego goal i bez nowych subagentów.

Repo D:\Dokumenty\Documents\RetroGadu, master, commity jako webs1821
<11637724+webs1821@users.noreply.github.com>. Workspace:
D:\Dokumenty\Documents\ChatGPT\Serwery. Repo wymaga eskalacji dla zapisów/build/git.
Brak AGENTS.md w repo i przodkach. PLAN_IMPLEMENTACJI.md jest niezmienny:
cc6fadfb16ab173d486b21aaa47eaf8c7843436fef8e67155a5333ae6613adec.

## Najnowszy stan

- 77d88ac: dev25, poprawka uploadu klienta GG6.1. Własny stub82B w128B paddingu,
  27 zakresów, pusty MORE dla dodatniej wielokrotności2047 i licznik ACK.
  Samodzielny walidator MIT/Unicorn2.1.4:79 przypadków CPU;8 sekwencji TCP/restart;
  pakiet3 hosty/464 zasoby Windows;6 porównań JS/Python. Całego GG nie uruchamiano.
  CLIENT-UPLOAD.md i raporty client-*-2026-11-01-dev25.json.
- 0dfd1b6329f15c9e763202ac80697fc46ff7bb00: dev26/schema13. Pełna historia100000
  powodowała EOF (109/200 odebranych,91 Pending,DB/FK ok). Nowy indeks częściowy
  messages_with_payload i korelowane purge/expiry/privacy usuwają skan historii
  z tych operacji. Retire czyści też conference. Limity i retencja bez zmian.
  103 Rust,fmt/Clippy,25 grup regresji/migracji i9 grup awarii przeszły.
  Prawdziwy backup dev25 (schema12) ->13: zachowane12 sum i wszystkie dane.
- 51c078cc8a5e2c2a329791fa4ef4ccf31b514ed5: korekta samej fixture pełnej historii.
  Kontekst sqlite3 nie zamykał połączenia, WAL14MB zostawał obok bazy14MB i źródeł,
  wypełniając tmpfs32MiB; metryki503/SQLITE_FULL. Dodano contextlib.closing.
  Ten sam runtime przeszedł poprawiony test przy tych samych limitach.
  container-dev26-attempt1.json zachowuje nieudaną próbę i diagnostykę.

Normalne binarki debug dev26 przywrócone po awariach:
server63f80a1c657000fb07d5b8a893eae6902a900cce8e4930c802d96fe3955f6b73,
admin b00fb3f370eb2a24ac8be6e1dfbed116bef3b2a7e21b8b5567e1cdfeeca9845e.
Historia-regresja dotyczy starej wersji fixture, co odnotowano; nowy sterownik
ma osobne dowody z obrazu. 190 bibliotek,10 crate,10 wpisów THIRD_PARTY.

## Ostatni kwalifikowany obraz

Źródło51c078cc8a5e2c2a329791fa4ef4ccf31b514ed5, dev26/schema13.
27 zestawów release84.912s, runtime restart/backup-restore oraz R0 przeszły.
R0:600.001s,100 Ready,6000 wiadomości (3000online+3000offline),1CPU/256MiB/swap0.
ACKp95=5.431ms,onlinep95=10.389ms,CPU0.119971,RSSpeak18849792B,cgrouppeak20975616B.
Po SIGTERM DB/FK ok,6000 TransportWrittenUnconfirmed,zero sesji/payloadów,cleanup ok.
GG6 nie ma ACK odbiorcy. To nadal próbka R0, nie długi soak/natywny klient/produkcja.
Suite/runtime miały swap=max; R0 ustawił i zweryfikował0 we własnej grupie.

Runtime sha256:1a6f76583208ce19a9aad4ba9f1d51cab803fbb21dc63658c860da6678fcbb59.
Verification sha256:3f0806df9532b2c448ee7f547b3060cdd6079d230a590a150334a42f05bf7e59.
Release-server9a83c9885ca7a0888d0e086a17d1c8ebbf4476a3c6ea444a7b045aee90d7c053.
Source archive285a8a912537541a5c0134142c582c6538f1aed96f2f38cb4ac26852fa970484.
Eksport workspace/rgg-container-2026-11-01-dev26/rgg-dev26-runtime.tar.gz,
61911804B, SHAe07315106bd55e8c87c2aef72477bda9e923400d0a3fb55b9cebc90ec9c39381.
GzipEOF/manifest/config identity sprawdzone. Sześć surowych raportów i manifest
container-artifact-2026-11-01-dev26.json są w repo oraz obok archiwum.
VERIFICATION-2026-11-01-r0-dev26.md zawiera opis i granice kwalifikacji.

Prywatny Docker ZATRZYMANY po eksporcie: zweryfikowany PID12, dawna sesja34969
wyszła0. Build27957, suite45425, R038995 i eksport85223 zakończone0.
Żaden test/daemon nie pozostaje aktywny. Cache, stare obrazy i archiwa zachowane.
Raporty i dokumentacja kwalifikacji26 są utrwalane osobnym commitem po51c078c.

## Następny krok

Zapowiedziano przygotowanie prywatnych metryk CPU/RAM procesu. Jeszcze NIE są
zaimplementowane w repo. W workspace są rgg-process-metrics-notes.txt oraz
rgg-process-metrics-draft.rs: projekt samplera Linux, parser VmRSS, szkic testów.
Przeczytać i przejrzeć, nie traktować szkicu jako przetestowanego kodu.
Rustix1.1.6 już jest pośrednio w lockfile; lokalnie sprawdzono bezpieczne
clock_gettime(ClockId::ProcessCPUTime)->Timespec. Potrzebny bezpośredni feature
"time" tylko dla Linux, bez własnego unsafe i bez zmiany wersji bibliotek.

Propozycja: app telemetry Copy ProcessSample z observed_at/CPU Duration/RSS Option;
server jeden sampler1/s tylko przy włączonych metrykach, cancellation/shutdown;
HTTP stałe rodziny CPU counter/RSS gauge/dostępność źródeł/wiek próbki.
Stara/nieobecna próbka nie może udawać zużycia0. Utrzymać limit16KiB odpowiedzi.
Parser ograniczony16KiB+1, nie wymaga UTF8 nazw procesu, kontroluje jednostki,
duplikaty i overflow. Dopracować lifecycle oraz API przed integracją.
Zależności/moduły i testy w istniejącym układzie; żadnych nowych subagentów.

## Narzędzia

Windows Python: C:\Program Files\Python313\python.exe; Node24.11.1:
C:\Program Files\nodejs\node.exe. Pliki Python czytać/pisać UTF8 jawnie.
WSL Python3.10. Rust1.99.1:
CARGO_HOME=/tmp/gg-retro-toolchain/cargo
RUSTUP_HOME=/tmp/gg-retro-toolchain/rustup
CARGO_TARGET_DIR=/tmp/rgg-target
CC=/tmp/gg-retro-toolchain/gcc/cc
PATH=$CARGO_HOME/bin:$PATH; cd /mnt/d/Dokumenty/Documents/RetroGadu.
/tmp/rgg-target/debug to zwykły dev26; /tmp/rgg-target/release nadal stary kandydat
writer oznaczony dev22. Nie utożsamiać go z nowym release z obrazu.

Prywatny silnik Docker29.9.2/runc1.5.2:
/tmp/rgg-docker-local-20261101, właściciel root0700; używać wsl -u root.
bin/,data29/,run29/,daemon29.json (containerd-snapshotter=false),docker29.sock,
daemon29.pid, ostatni log daemon-dev26.log. Bez bridge/iptables/IP forwarding,
tylko własny socketUnix; Docker Desktop i usługi systemowe nietknięte.
Build z czystego HEAD przez tools/build_container.py, brak nadpisywania tagów.
Root raporty końcowe26: runtime-dev26-final-build.json,verification-dev26-final-build.json,
suite-dev26-final.json,runtime-dev26-final-qualification.json,r0-dev26-plan.json,
r0-dev26-600s.json. Pierwszych buildów/logów26 nie usuwać.
Nie drukować całego R0 JSON: tylko wybrane scalars, bo ma setki próbek.

Baseline stare binarki: /tmp/rgg-baseline-dev8(schema6),dev9(7),dev13(8),dev14(9),
dev15(10),dev20(11),dev23/dev24/dev25(12). Dev25 server1087e5ee538c519e1ae261bef6bee64f4fa5a624154745afdfa71cbd03b356fe,
admin a7af13bd5f5fcbbcf66b7039659121e3fbd680ee32697e58295ef9d880f1fc9e.
Backup starego schematu odtwarza stary admin, potem nowy runtime migruje kopię.
sqlite_upgrade.py wymaga teraz --expected-schema13. Nigdy nie fabrykować historii
migracji przez ręczne zmiany _sqlx_migrations.

Starsze ważne artefakty/problemy: dev22 R0 nie przeszedł, dev23 writerFIFO naprawił
równoczesny replay, dev24 aktualizował SQLx0.9.0/libsqlite3-sys0.37.0/SQLite3.51.3
(WAL-reset fix). SQLITE-WRITER.md i SQLITE-UPGRADE.md zawierają szczegóły.
Archiwa dev18/dev19/dev24 oraz nieudane próby dev22 zachowane w workspace.

Pakiet klienta dev25: workspace/rgg-client-local-2026-11-01-dev25.
GG.exe b5e5c51f61467a07a0dce1079c45bfd4596cb3659aeb8a5b37843ad53faaaaeb,
installer166f31d95505a090750a7f0da09fe93ed4067e694dda6e6c88a57a3b7aaeb833.
Oryginał wejściowy: D:\Dokumenty\Documents\gg-retro\server\static\gg61.exe.
Analiza/prototypy/Unicorn tylko workspace/rgg-client-analysis-2026-11-01.
Nie włączać oryginalnego klienta, disassembly ani innych binariów do Git.
