# Fakturácie

Osobná lokálna aplikácia v slovenčine. Adresár zákazníkov, úpravy priamo na faktúre, automatické ukladanie konceptov a PDF s platobným QR. Bez platených API.

## Spustenie cez Docker

Aplikácia, PHP 8.5, Apache aj SQLite bežia v Docker Desktop. PHP verziu určuje základ `php:8.5-apache-bookworm` v `Dockerfile`; `compose.yaml` z neho zostaví aplikačný obraz. Na Macu nepotrebuješ PHP, Composer ani Node. Otvor **Spustit.command**, prípadne v priečinku projektu:

```sh
docker compose up -d --build --wait
```

Pri prvom spustení sa vytvorí prázdna databáza. Najprv v **Moja firma** vyplň údaje a účet dodávateľa, potom pridaj zákazníkov. Repozitár neobsahuje vzorové faktúry ani firemné údaje. Existujúca databáza sa pri spustení zachová.

Port môžeš voliteľne zmeniť cez `.env`: skopíruj `.env.example` a uprav `FAKTURACIE_PORT`. SMTP nastavuješ priamo v aplikácii.

Adresa je `http://127.0.0.1:8873`, prípadne `http://faktura.local:8873`, ak lokálny DNS alebo `/etc/hosts` smeruje `faktura.local` na `127.0.0.1`. Kontajner beží na pozadí, okno terminálu môžeš zatvoriť. V Docker Desktop ho nájdeš v projekte **fakturacie**, služba **app**. `restart: unless-stopped` ho obnoví po opätovnom spustení Dockeru, pokiaľ si ho manuálne nezastavil. Docker Desktop musí bežať.

```sh
docker compose stop       # zastavenie bez vymazania dát
docker compose up -d --wait
docker compose ps
docker compose logs --tail=100 app
```

Je to Docker na tomto Macu, dostupný iba cez loopback. Vzdialený hosting, verejný prístup a prihlasovanie nie sú súčasťou tejto konfigurácie. Port je zámerne publikovaný na `127.0.0.1`; v kontajneri sa zohľadňuje Docker NAT a ďalej sa kontroluje Host, Origin a CSRF.

Obraz sa zostavuje z lock súborov a zdrojov. Databáza, faktúry, zálohy ani lokálne cache sa do obrazu neposielajú. Po zmene kódu spusti znovu `docker compose up -d --build --wait`.

## Používanie

1. V Zákazníkoch si vytvor alebo uprav firmu, prípadne ju pridaj priamo pri výbere odberateľa.
2. Zvoľ Nová faktúra. Klikni na údaje na papieri A4 a uprav dátumy, položky, množstvo, ceny alebo poznámku. Koncept sa automaticky ukladá. Na mobile sa dá dokument priblížiť.
3. Vyber najviac dva QR formáty. Pri nevyplnenom alebo nesprávnom údaji aplikácia zobrazí chybu.
4. Vystaviť a stiahnuť PDF pridelí číslo a uloží konečný dokument. Aj vystavenú faktúru môžeš upraviť cez Upraviť faktúru → Uložiť zmeny. Zachová sa jej číslo a stav; PDF aj QR sa vytvoria z upravených údajov. Zmeny vystavenej faktúry sa uložia až týmto tlačidlom. Vytvoriť kópiu pripraví nový koncept.

Kópiu a vymazanie nájdeš v ponuke **Ďalšie**. Každú faktúru môžeš bez ohľadu na stav vymazať tlačidlom **Vymazať faktúru**. Po potvrdení sa odstráni aj uložené PDF. Vymazané číslo sa znova nepoužije; číselný rad sa nevracia späť. Súbežné úpravy alebo vymazanie zo starej stránky sa odmietnu konfliktom verzie.

Editor a nové PDF používajú spoločnú šablónu dokumentu a rovnaké písmo DejaVu Sans. Prázdne editovacie pomôcky a ovládacie tlačidlá sa netlačia. Pri dlhom dokumente skontroluj rozdelenie strán cez náhľad PDF. Platobný QR kód vznikne až po pridelení čísla; dovtedy jeho miesto označuje plocha „QR po vystavení“.

Číslo sa prideľuje až pri vystavení. Nová inštalácia začína číslom `RRRR0001`; každý rok má samostatný rad. Obnova zálohy zachová uložené čísla aj nasledujúcu hodnotu radu. Testy používajú oddelenú databázu.

Táto verzia podporuje EUR a ručný výber DPH 0, 5, 19 alebo 23 %. Základ a DPH sa zaokrúhľujú po položkách na centy; sadzby sa následne sčítajú. Daňový režim zahraničného plnenia sa neurčuje automaticky.

## QR platby

- **PAY by square**: slovenský formát. Kontroluje sa IBAN, suma a referencia. Voliteľný deň vykonania platby sa nevkladá, aby kód nezostarol po splatnosti.
- **QR Platba / SPAYD**: český formát. Podporu EUR a slovenského IBAN-u musí mať banka zákazníka; univerzálna česká podpora sa týka tuzemských CZK platieb. Limit sumy tohto formátu je 9 999 999,99 EUR.
- **SEPA QR / EPC**: EUR platby v bankových aplikáciách, ktoré tento formát čítajú.

Podpora v každej banke EÚ, Revolute a Wise nie je potvrdená. Všetky faktúry preto obsahujú aj čitateľný IBAN, sumu, menu a referenciu pre manuálny prevod. QR sa generuje lokálne, bez odoslania údajov do cudzej služby.

Špecifikácie: [PAY by square](https://portal.bysquare.com/files/bysquare-PAYspecifications-1.2.0.pdf), [QR Platba](https://qr-platba.cz/pro-vyvojare/specifikace-formatu/), [EPC](https://www.europeanpaymentscouncil.eu/document-library/guidance-documents/quick-response-code-guidelines-enable-data-capture-initiation-sepa).

## Dáta a záloha

Aktívna databáza je v Docker volume **fakturacie_data**, v kontajneri `/var/www/html/data/fakturacie.sqlite`. Obsahuje zákazníkov, nastavenia, koncepty aj konečné PDF. Prežije reštart, nový build aj opätovné vytvorenie kontajnera. Príkaz `docker compose down -v` odstráni aj volume s dátami; na bežné zastavenie používaj `stop` alebo `down` bez `-v`.

Súbor `data/fakturacie.sqlite` na hostiteľovi, ak existuje, nie je aktívna databáza Docker aplikácie. Aktívne dáta sa ukladajú do pomenovaného volume.

Konzistentnú zálohu aktívnej databázy do `data/backups` vytvoríš aj počas prevádzky:

```sh
./bin/docker-backup.sh
```

Skript používa SQLite `VACUUM INTO` v kontajneri. Záloha obsahuje aj uložené PDF. Cache, sessions a technické logy v kontajneri sú dočasné; po jeho vytvorení sa obnovia.

Obnova na novom počítači alebo do **prázdneho** úložiska (nahraď cestu ku konkrétnej zálohe):

```sh
docker compose build
docker compose run --rm -T --no-deps --entrypoint php app docker/database.php import < cesta-k-zalohe.sqlite
docker compose up -d --wait
```

Import spusti ešte pred prvým spustením aplikácie do prázdneho volume. Overí integritu a odmietne prepísať existujúcu databázu. Prázdne úložisko sa samo nevyplní vzorovými faktúrami. Pri obnove existujúcej inštalácie najprv zálohuj aktuálne dáta; pôvodný volume nemaž bez overenej zálohy.

`.gitignore` vylučuje `.env` a jeho varianty, lokálne konfigurácie, databázy, PDF, e-maily, screenshoty a exporty v `output`, zálohy, logy, testovacie kľúče a závislosti. Sledovaný `.env.example` obsahuje iba port. SMTP heslo sa ukladá do privátnej databázy. Docker build kontext povoľuje iba zdroje a potrebné konfiguračné súbory.

## Posielanie e-mailov a SMTP

V **Moja firma → Odosielanie e-mailov** nastav server, port, STARTTLS alebo SSL/TLS, používateľa, heslo, adresu odosielateľa a prípadne Reply-To. Prázdne heslo ponechá uložené heslo; pri zmene servera alebo používateľa ho treba zadať znova. Voľba Bez TLS slúži napríklad pre lokálny Mailpit. Certifikáty TLS sa overujú, kontrola sa nedá vypnúť v rozhraní. Niektorí poskytovatelia vyžadujú heslo aplikácie alebo nepovoľujú prihlasovanie bežným SMTP heslom; OAuth nie je súčasťou tejto verzie.

Po uložení môžeš poslať skúšobný e-mail na vlastnú adresu. Vystavená faktúra má tlačidlo **Poslať e-mailom**: skontroluj príjemcu, predmet, text a náhľad údajov, potom odošli. Príjemca sa predvyplní zo zákazníka, ak má uložený e-mail. PDF je skutočná príloha; vybrané QR sú vložené PNG obrázky cez Content-ID. Správa obsahuje sumu, splatnosť, IBAN, BIC a variabilný symbol aj v textovej alternatíve. Nepotrebuje vzdialené obrázky ani odkaz na tento počítač. Koncept treba najprv vystaviť. Po úprave vystavenej faktúry odošli novú správu s jej aktuálnou verziou.

Pod faktúrou je história posledných desiatich pokusov. „SMTP server prijal správu“ potvrdzuje prijatie serverom, nie doručenie do schránky alebo prečítanie. Rovnaká požiadavka sa neposiela opakovane ani po strate odpovede. Pri nepotvrdenom výsledku skontroluj server a schránku pred otvorením nového pokusu. Do histórie sa ukladá príjemca, predmet, čas, verzia faktúry a odtlačok PDF. Odoslané správy nemajú účinok na stav alebo číselný rad faktúry.

SMTP nastavenia vrátane hesla sú v privátnej SQLite databáze Docker volume a v jej zálohách. Heslo sa neposiela späť do prehliadača ani do aplikačných logov; databáza nie je dostupná cez web. Zálohy preto obsahujú aj prihlasovacie údaje. Táto verzia ich nešifruje samostatným externým kľúčom.

### Skúška cez Mailpit

Oddelený projekt `fakturacie-email-verify` používa vlastnú databázu a Mailpit na `http://127.0.0.1:8026`; server z iných projektov sa nemení. Mailpit neprenáša správy zákazníkom. Testovacia konfigurácia je v `docker/compose.mailtest.yaml` a nepatrí do pracovného Compose spustenia.

```sh
docker build --target runtime -t fakturacie:email-test .
FAKTURACIE_PORT=8874 docker compose -p fakturacie-email-verify -f compose.yaml -f docker/compose.mailtest.yaml up -d --wait
npm ci
mkdir -p temp/verification
node tests/email-http.mjs
```

HTTP testy overujú SMTP probe, súbežné a opakované odoslanie, vlastného príjemcu/predmet/text, odmietnutie starej verzie faktúry alebo konfigurácie, históriu a neúspešné spojenie. `tests/email-domain.php` kontroluje validáciu nastavení, prácu s heslom a MIME správu. Zachytený `.eml` je v `temp/verification`; príloha sa porovnáva s pôvodným PDF a QR obrázky sa dekódujú nezávisle. `tests/smtp-security.php` overuje nedôveryhodný certifikát, STARTTLS, implicitné TLS a nesprávne heslo proti dvom ďalším Mailpit serverom v profile `security-test`. Tento profil potrebuje vlastný krátkodobý certifikát s menami `mailpit-tls` a `mailpit-ssl` v `temp/mailpit-tls` a bcrypt súbor `smtp.htpasswd`; testovacia CA sa pridáva iba do dočasného testovacieho kontajnera, nie do pracovného obrazu.

## Technický základ a overenie

Nette 3.3, Latte 3.1, Bootstrap 5.3, SQLite, mPDF, Endroid QR a lokálny encoder PAY by square. Zamknuté verzie sú v lock súboroch. Presentery sprostredkúvajú HTTP; doménové služby riešia validáciu, výpočty, číslovanie a vystavenie. Vystavenie a uloženie PDF prebiehajú v jednej SQLite transakcii. Ochrana zápisov používa CSRF token, kontrolu pôvodu a verziu záznamu.

Kontrola PHP v samostatnom testovacom obraze (bez pripojenia pracovného volume):

```sh
docker build --target test -t fakturacie:test .
docker run --rm --entrypoint php fakturacie:test tests/run.php
docker run --rm --entrypoint php fakturacie:test vendor/bin/phpstan analyse --debug --no-progress --memory-limit=512M
```

HTTP testy potrebujú samostatný projekt, port 8874 a Node na testovacom hoste. V oddelenej databáze si nastavia syntetického dodávateľa a vytvoria vlastné testovacie faktúry. Pracovný projekt sa volá `fakturacie`:

```sh
FAKTURACIE_PORT=8874 docker compose -p fakturacie-verify up -d --build --wait
npm ci
mkdir -p temp/verification
node tests/http.mjs
FAKTURACIE_PORT=8874 docker compose -p fakturacie-verify down
```

Kontroly zahŕňajú centové výpočty, miešané DPH, neplatné dátumy a IBAN, súbežné vystavenie, opakované požiadavky, konflikty verzií, úpravy a mazanie oboch stavov, súlad opraveného PDF/QR, zachovanie čísla a nový ročný rad a HTTP ochrany. PDF sa navyše vizuálne kontrolovalo vrátane dlhého dokladu. QR z vytlačených strán sa načítali systémovým dekodérom a PAY payload sa nezávisle dekomprimoval a overil cez CRC.

Sass téma v `assets/scss/app.scss` nastavuje modrú primárnu farbu, typografiu a stavové farby `issued` / `draft`, ktoré generujú Bootstrap triedy `text-bg-issued` a `text-bg-draft`. `assets/invoice.css` obsahuje spoločné fyzické rozmery dokumentu pre editor a PDF.

Build obrazu zostaví štýly, JavaScript aj zhodné fonty pre editor a PDF. Cache aplikácie vzniká až v kontajneri.

WebAmulet, párovanie úhrad a verejné nasadenie sú odložené na ďalšiu etapu.

Docker konfigurácia vychádza z [oficiálneho PHP obrazu](https://hub.docker.com/_/php) a [Docker volumes](https://docs.docker.com/engine/storage/volumes/).
