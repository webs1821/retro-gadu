# Punkt wznowienia — 2026-11-02, dev.30

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

AKTYWNA KWALIFIKACJA OBRAZU30 (nie kończyć pracy po przeczytaniu):
Dev30commit90aed8d2dab49790a9f59ebd105596f2d5d20ae4zakończony; prywatnyDocker
DZIAŁA wsession13886, root/tmp/rgg-docker-local-20261101(data29,run30,piddaemon30.pid,
socketdocker30.sock,logdaemon-dev30.log). Brakbridge/firewall/publicports/systemservice.
Starterworkspace/rgg-container-start30.py; preflightsprawdziłroot0700uid0/basestate.
Buildsession19921zakończony0: runtime5990b4b936ba07d51d94120b64c71681eca27de1702dd9be248c1956b9b06ea5,
verificationcaed86782486771fe6cebc9e9516df6a9251f09bb25a57831a4a06e156d95a3c.
Rawroot/runtime-dev30-build.json,verification-dev30-build.json +logs zachowane.
Suite session38675FAILED atgg6_history_retention configwrite ENOSPC before server.
Root suite-dev30-failure.log andtmpfs-dev30-diagnostic.json, archived repo
container-dev30-attempt1.json. Failedcontainerscleaned byEnginefinally.
Sourceextract consumes5414912Btmpfs; seedDB13946880+WAL14152232 leavesfree0.
Sameimage withoutsourceextract seedDB14221312/WALgone,free19329024,configwritePASS.
Driverworkspace/rgg-container-space30.py. NOT a reappearance of unclosedfixture:
connection closing still present, temporary space forcheckpoint now insufficient.
HARNESSONLYfix deploy/Dockerfile verification tar extractsnonvendoredsource toRO/src
atbuild insteadCOPYtests/tools. tests/container_suite.py uses /src directly,
assertsRO andnovendor/.cargo; reportsread_only_test_sources etc. NoRuntimecode,
version/schema/deps/limitschanged. Needcommitfix,buildnewsource/images withnew
-final reportnames,qualify32groups/runtime,thenR0/export. Preparedworkspace
rgg-container-build30.py/qualify30.py/r030.py/export30.py currentlypointold90source
andnonfinalreportnames; createadaptednewdrivers, neveroverwriteoldreports/tags.
R0notstarted. Exportnotstarted. NeedstopownDockerafterexport, keepcaches/images.
RepoDOCS/RESUMEchangehereonlycheckpointofworkinprogress; reflectsfailedfirstimage.


Dev29 ukończony w b412748. Dev30 zaimplementowany i zweryfikowany lokalnie:
web_access.rs: WebProxy typedSerdeTryFrom canonicalHTTPSorigin+oneLoopbackProxyIP;
WebEndpoint::new(bound,Option<WebProxy>) nadal loopback/nonzeroport. Website
używa Endpoint zamiastSocketAddr. Sharedguard tworzy Extension WebsiteClient
zweryfikowanegoIP, obaformularze używająextensiondoCSRF/budżetu, Secure także
clearcookies. Host/Origin/FetchSite pojedyncze/dokładne; GETOriginjeślijestteż
musi byćzgodny. XRealIP singleplainIP + XForwardedProtohttps tylko odexactpeer;
XFF/Forwarded/XForwardedHostignorowane. MappedIPv6clientnormalizowanydoIPv4;
mappedIPv6originodrzuconyjakaliasy/slash/query/userinfo/default443/invalidport.
DOMDNS lowercaseLDH/253/63finalalphaonly (IDNpunycodeTLDunsupported). NoDNS.
Configweb_proxy wymagaenabledwebsitefeature, bindpublicnadalzabroniony.
http_proxy.py usesnewlocalPythonTLSproxy1joinedworker, OpenSSLCLIephemeralcert,
cert+hostnameverify, noDNS/systemtrustchanges. ClientA/B testsCSRF, Secure,
registration/passwordfence/GG, downloads/patcherexactbytes; spoofheaders,
IPaliases/budgets, noDBchanges;13badconfigs beforeDB. TLS1.3success+negativeCA/name.
123Rust/fmt/Clippy/build+12groupsTCP/HTTP allPASS;2container_context też.
Report web-proxy-2026-11-02-dev30.json74.736s hashessourcesverified.
Debug30 serverdaf5b49fec22be520fa68d66a39ffc7b88bc4f01482744158c350d4531b84b3b,
admin3e4fcab3667ee82386f390240da8bd946fe73623c8673eee15f285ce698292b7.
190deps/schema13 unchanged,12provenance (newhttps-proxy-reference primaryRFC/
nginx facts),10crate. CI/container_suite32groups; Dockerverification now RUN
openssl version; futurebuildwillverifyCLIpresence (noruntimepackagesadded).
Noimage/R0/nginx qualification; image26 remainslatest. Noactiveprocess/test.
Runnerworkspace/rgg-web-proxy30.py refusesoverwritingreport. Previouspreflight
TLSfixturetriedinviteafterserverstart,correctexclusiveDBlockrejected; fixed
fixtureonly toinvitebeforestart; finalfullreportpasses. Nootherpendingissue.
DocsWEB-PROXY,VERIFICATIONdev30,STATUS/README/PARITYupdated. Need localcommit
ofdev30 ifnotyetdone, thencontinueautonomouslocalworkbyremainingplan.

Dev28 ukończony w16373ff. Dev29: ImageObservations/ImageSnapshot port application,
Images::image_snapshot try_lock + skan<=32 bez alokacji/I/O/mutacji. Obserwuje ten
sam Arc koordynatora co GG. HTTP metrics::router ma dodatkowy Option<Arc<dyn
ImageObservations>>; None gdy brak GG.6 nowych rodzin/9serii, całość27/maks126,
<16KiB. Unavailable0 + pominięcie8serii przy busy/poison/noGG. Stan, rezerwacje,
len/capacity danych+nazwy i monotoniczny najstarszy wiek; bez outgoingkopii.
119Rust,fmt,Clippy,normalbuild i8grup TCP/HTTP PASS. image_metrics.py ~33s zawiera
prawdziwydeadline30,458s/140migawek, restart, quota4, oba disconnect, noGG.
Pierwszy report image-metrics-2026-11-02-dev29.json zachowuje błędne oczekiwanie
testu quota6+missing (runtime poprawnie4/noresponse). Poprawiono wyłącznie test,
final report -final.json powiązany hashem z pierwszym; runtime/binarki ponownie
sprawdzone, wcześniejsze udaneRust/buildniepowtarzane. Wszystkie sourceSHAsprawdzone.
Binarne debug29: server55e176f6c0e47a97f0f7a8d0061f94ed51dcb466df9e0dd611932d407ff4d26a,
admina4d48c5441016ced693458c32817118e0783c7aefced41293a199fae62872ac5.
Schema13/190deps/11provenance/10crate bez zmian. CI/container_suite31grup.
Obraz/R0 nadal26; wszystkie testy zakończone. Workspace runner
rgg-image-metrics29.py i -final.py; raportów nie nadpisywać.
Historycznie następny zapowiedziany krok (już wykonany wdev30): konfiguracja WWW za proxy HTTPS testowanym lokalnie,
ze ścisłym canonical origin/Host/CSRF/cookie, jawnie zaufanym proxy i źródłemIP.
Nie wdrażać ani nie otwierać publicznych nasłuchów. Implementacja tego kroku jest zakończona; bieżący stan opisany powyżej.

Macierz stanów sesji utrwalona w67eaf97ea153165bd5b812fac372793fe121130a.
Dev28 dodaje opcjonalne gg_advertise: walidowany rgg_http::AdvertisedEndpoint,
numerycznyIPv4/nonzeroport, odrzuca0/8 i>=224. Konfiguracja wymaga discovery oraz
lokalnego IPv4gg_bind. Nie zmienia public-bind guard, DNS, Host ani WWW/TLS.
HttpState::with_legacy_discovery ma drugi argument Option<AdvertisedEndpoint>.
W tests/gg6_sessions.Lab.start dodano opcjonalny gg_advertise=None (domyślny tekst
konfiguracji bez zmian). discovery_proxy.py: bounded8par relay127.0.0.2->127.0.0.1,
HTTP1.0/hostileheaders, online/offline, restart z nowym portem zaplecza;15 złych
konfiguracji przed DB.113Rust,fmt/Clippy,discovery+proxy i5 regresji PASS.
Raport advertised-endpoint-2026-11-02-dev28.json rozróżnia wcześniejsze wyniki
narzędzia od pięciu dodatkowych grup z pełnym stdout/stderr. Sumy źródeł sprawdzone.
Nowy normalny debug28:
server22efb6a346685965175c7a2d414115630a2c4ef5bb7ee866f6fc0be80ff5e948,
admin631c1caad9d265f940fd947491d967731834fee70bcbba856d9a4c0b8bc4d80f.
Baseline27 zweryfikowany i zachowany w/tmp/rgg-baseline-dev27.190 bibliotek,
11provenance,10crate,schema13 bez zmian. CI/przyszły container_suite30grup.
ADVERTISED-ENDPOINT.md iVERIFICATION-2026-11-02-dev28.md. Publiczny profil WWW/TLS
nadal niezaimplementowany/niekwalifikowany. Brak nowego obrazu, push lub deploy.
Wszystkie serwery/relaytesty zakończone. Runner workspace/rgg-advertise28.py nie
nadpisuje raportu. Ostatni kwalifikowany obraz pozostaje dev26 z archiwum poniżej.

Testy mutacyjne są zapisane w fc796b925a5ff89c6dfb464d9bf77bd0d305c726.
Kolejny osobny krok testowy: tests/session_contracts.py,42 komendy w złych stanach
(15 przed auth,13 przed Ready,14 resynchronizacja). Wcześniejsze blokady/profil/
archiwum i kolejka pozostają bez zmian; inne sesje i poprawne późniejsze operacje
są sprawne. Ping nie przedłuża deadline1s. Raport session-contracts-2026-11-02.json
PASS, hash sterownika i binarek sprawdzony. SESSION-CONTRACTS.md opisuje ograniczenia.
CI/container_suite zawiera ten test (przyszłe suite29grup). Rust nadal111testów,
runtime/dev27/schema13/dependencies bez zmian. Testy zakończone, brak procesów.

Commit2a58f395c86c3f27cc65ff7a506e3fd992a1ce63 kończy implementację dev27.
Następny osobny commit testowy dodaje crates/rgg-protocol/tests/adversarial.rs
oraz fixtures/wire-corpus.json:41 własnych wzorców,11545 prefiksów,92032 bit-flipy,
2mln mutacji/8seedów. Limity/przyjęte wyniki i niezależne ramkowanie sprawdzone.
111Rust,fmt/Clippy PASS. Runtime/dependencies/schema bez zmian, binarki debug27
mają nadal sumy poniżej. Domyślne20k mutacji uruchamia cargo test; limit env
RGG_PROTOCOL_CASES1..2mln, niezerowy RGG_PROTOCOL_SEED. Własny PRNG tylko do testów.
Raporty protocol-mutation-2026-11-02-final-plan.json oraz -final.json zweryfikowane.
Pierwsze plan.json i .json zachowują preflight z błędami Clippy chunks_exact;
nie było wtedy rozszerzonego fuzzingu. Końcowy probeSHA9231c45d43a53a6ccdf14ed1b5bff76ab621bef113efd880269ee7eaeaa873fc.
Workspace runners rgg-protocol-mutation.py (pierwszy) i -final.py (końcowy),
nie uruchamiać ponownie pod tymi samymi raportami. Wszystkie procesy zakończone.
PROTOCOL-MUTATION.md dokumentuje zakres: bez coverage guidance/sanitizerów,
bez sekwencji złych stanów sesji, dekompresji czy klienta natywnego.

Dev.27 kończy opcjonalne prywatne CPU/RSS: rgg-server/process_metrics.rs,
ProcessSample w application/telemetry, cztery rodziny w HTTP. Jeden sampler1/s,
Skip, age od początku odczytów, stara próbka>5s i brak źródła mająavailability0
bez udawanego użycia0. CPU przez fallible clock_gettime_dynamic(Known(ProcessCPUTime)),
RSS bounded16KiB+1/proc/self/status; parser bajtowy/kB/duplikaty/overflow.
Sampler tylko z metrics_bind, cancellation i join przy shutdown. Bez własnego
unsafe; bezpośrednia Linux rustix1.1.6/time,190 zewnętrznych wersji bez zmian.
108Rust,fmt,Clippy i7 grup procesowych PASS: process_metrics,http_metrics,
runtime_metrics,lifecycle,gg6_history_retention,gg6_concurrent_replay,r0_smoke.
Raport process-metrics-2026-11-01-dev27.json; opis VERIFICATION-2026-11-02-dev27.md.
21 rodzin/maks117 szeregów<16KiB. CI i przyszły container_suite mają teraz28grup.
11 wpisów pochodzenia,10crate,13migracji. Normalny debug27:
server089c2d0346289e57407b109cd1bcc38ee4c0110348f62a7f6f00c17c9e346555,
admin ccc8c5b6e1fa92e959fd84ee04246e442fd41892087bd546b0e96e89c2748022.
Raporty obrazu26 utrwalono commitem22b0932. Dev27 jest kolejnym lokalnym commitem;
bez nowego obrazu/R0/soak. Brak aktywnych procesów testowych i pytań do użytkownika.

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

Historyczne binarki debug dev26 po awariach (obecne debug27 opisano wyżej):
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

## Kolejne prace

Kontynuować autonomiczny rozwój lokalny do przerwania przez użytkownika.
Sprawdzić pozostałe braki PARITY i planu; pozostały m.in.
kwalifikacja publicznego proxy, natywny GG, rzeczywisty import i długi soak.
Nie zmieniać zakresu lokalnego ani uruchamiać istniejącego GG/instalatora
na jego bieżącym profilu. Nie uruchamiać subagentów bez nowej podstawy.
Szkice CPU w workspace są już zastąpione finalnym kodem w repo.
Procesowy runner testów: workspace/rgg-process-metrics27.py; nie nadpisuje raportu.

## Narzędzia

Windows Python: C:\Program Files\Python313\python.exe; Node24.11.1:
C:\Program Files\nodejs\node.exe. Pliki Python czytać/pisać UTF8 jawnie.
WSL Python3.10. Rust1.99.1:
CARGO_HOME=/tmp/gg-retro-toolchain/cargo
RUSTUP_HOME=/tmp/gg-retro-toolchain/rustup
CARGO_TARGET_DIR=/tmp/rgg-target
CC=/tmp/gg-retro-toolchain/gcc/cc
PATH=$CARGO_HOME/bin:$PATH; cd /mnt/d/Dokumenty/Documents/RetroGadu.
/tmp/rgg-target/debug to zwykły dev28; /tmp/rgg-target/release nadal stary kandydat
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
