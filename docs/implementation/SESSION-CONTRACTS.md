# Komendy GG6 a stan sesji — kontrola lokalna

Test `tests/session_contracts.py` sprawdza42 przypadki przez rzeczywiste TCP:
15 przed uwierzytelnieniem,13 po LOGIN_OK przed pierwszą synchronizacją oraz14
po ponownym NOTIFY_FIRST, gdy nowa lista prywatności pozostaje w stagingu.

Komendy obejmują tekst, konferencję, ofertę/żądanie/fragment obrazka, własny
profil, zapis i wyszukanie w katalogu, pobranie/zapis archiwum oraz dodanie/usunięcie
kontaktu. Przed loginem także ping, status i EMPTY. Po loginie sprawdzany jest
powtórny LOGIN60, a podczas resynchronizacji EMPTY zamiast prawidłowego LAST.

Pakiety są poprawnymi przykładami formatu; naruszają kolejność operacji.
Przed loginem oczekujemy LOGIN_FAILED i zamknięcia, a po loginie kontrolowanego
zamknięcia bez sukcesu komendy. Po każdej próbie porównujemy w spójnej transakcji
wiadomości, dostarczenia, oferty obrazków, profil, archiwum, trwałe kontakty
i limit katalogu. Sprawdzamy brak pozostałego stagingu oraz działanie innej
sesji. Wcześniej zapisana blokada nadal musi odrzucać wiadomość.

Test rozróżnia prawidłowe operacje synchronizacji: ping i zmiana lokalnego opisu
są dozwolone po LOGIN_OK, ale opis pojawia się w bazie dopiero po Ready.
Po całej macierzy poprawna synchronizacja, odczyt profilu/archiwum i nowa
wiadomość nadal działają. Integralność SQLite i klucze obce są poprawne.

Osobny przypadek wysyła ping co100ms przy limicie synchronizacji1s.
Połączenie musi zakończyć się w tolerancji0,8–2,5s, mimo kolejnych PONG;
aktywność nie może przesuwać bezwzględnego deadline. Inna sesja działa nadal.

Wynik z2026-11-02: wszystkie przypadki przeszły na zwykłych binarkach dev.27.
Raport `session-contracts-2026-11-02.json` zawiera macierz, czasy i hashe sterownika,
serwera oraz administratora. Dodano test do CI i przyszłej kwalifikacji obrazu
(po tym kroku container_suite miał29 grup). Ostatni kwalifikowany obraz pozostaje dev.26.

To własny deterministyczny test wybranych stanów, używający istniejącego,
przypisanego w THIRD_PARTY portu wyroczni GG32. Nie stanowi pełnego fuzzingu
sekwencji ani testu natywnego GG. Nie zmieniono kodu runtime, zależności i schematu.
