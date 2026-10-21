# Fakturácie

Osobná aplikácia v slovenčine: adresár zákazníkov, interaktívny editor priamo na faktúre, automatické ukladanie konceptov, PDF s platobným QR a odosielanie cez vlastné SMTP. Bez platených API.

Backend je v **Ruste**, používa **Axum**, **SQLite**, **MiniJinja** a **Lettre**. PDF vykresľuje **Chromium** zo spoločnej HTML šablóny editora. JavaScript a Bootstrap 5.3 zostávajú vo fronte. Celá aplikácia vrátane PDF renderera beží v Dockeri; na Macu nepotrebuje PHP, Rust, Composer ani Node. Node slúži iba pri zostavení frontendových súborov v build kontajneri.

## Spustenie

Spusti Docker Desktop a otvor **Spustit.command**, prípadne v priečinku projektu:

```sh
docker compose up -d --build --wait
```

Otvor **http://127.0.0.1:8873**, prípadne **http://faktura.local:8873**, ak lokálny DNS alebo `/etc/hosts` smeruje `faktura.local` na `127.0.0.1`. Port môžeš zmeniť cez `.env` podľa `.env.example`.

Hlavný Compose je **`compose.yaml` v koreni projektu**. `Dockerfile` zostaví Rust server a frontend, výsledný runtime obsahuje server a Chromium. PHP, Apache, Node ani Rust toolchain v pracovnom kontajneri nie sú. Server v kontajneri počúva na porte 8080; Compose ho publikuje na `127.0.0.1:8873`. Kontajner beží ako `www-data`, Chromium sa spúšťa iba počas vytvárania PDF a potom sa ukončí. PDF nepoužíva externú renderovaciu službu a má zablokované sieťové zdroje.

```sh
docker compose ps
docker compose logs --tail=100 app
docker compose exec -T app fakturacie --version
docker compose stop
docker compose up -d --wait
```

`restart: unless-stopped` obnoví aplikáciu po spustení Dockeru, pokiaľ si ju manuálne nezastavil. Po zmene zdrojov zopakuj `docker compose up -d --build --wait`.

Je to Docker na tomto počítači, dostupný iba cez loopback. Vzdialený hosting a prihlasovanie nie sú súčasťou tejto konfigurácie. Server kontroluje Host, Origin, CSRF a verziu upravovaného záznamu; cookies majú HttpOnly a SameSite=Strict. Po vytvorení nového kontajnera otvor stránku znovu, pretože relácie sú dočasné.

## Používanie

1. V **Moja firma** vyplň dodávateľa a bankový účet. Nová inštalácia obsahuje prázdnu databázu bez firemných údajov.
2. V **Zákazníkoch** vytvor firmy alebo zákazníka pridaj priamo pri výbere odberateľa.
3. Otvor **Nová faktúra**, klikni na údaje na papieri a uprav dátumy, položky, množstvo, cenu a poznámku. Koncept sa automaticky ukladá. Na mobile sa dá dokument priblížiť.
4. Vyber najviac dva QR formáty. Platobný QR vznikne po pridelení čísla.
5. **Vystaviť a stiahnuť PDF** pridelí číslo a uloží dokument. Koncept má samostatný PDF náhľad.

Každú faktúru môžeš bez ohľadu na stav **upraviť aj vymazať**. Vystavenú faktúru upravíš cez **Upraviť → Uložiť zmeny**; zachová sa číslo aj stav a vytvorí sa aktualizované PDF a QR. Kópiu a vymazanie nájdeš v ponuke **Ďalšie**. Vymazaním sa odstráni aj uložené PDF; číslo sa opätovne nepoužije. Súbežné úpravy alebo mazanie zo starej stránky sa odmietnu konfliktom verzie.

Editor a nové PDF používajú spoločnú šablónu a rovnaké písmo DejaVu Sans. Prázdne editovacie pomôcky a tlačidlá sa netlačia. Dlhé dokumenty sa rozdelia na strany s opakovanou hlavičkou položiek a číslami strán; výsledok skontroluj v PDF náhľade.

Číslo sa prideľuje až pri vystavení. Nová inštalácia začína `RRRR0001`; každý rok má vlastný rad. Obnova zálohy zachová uložené čísla aj nasledujúcu hodnotu radu. Kópia používa aktuálne údaje firmy a zákazníka, uložené faktúry si ponechávajú svoje pôvodné údaje.

Podporovaná mena je EUR a ručný výber DPH 0, 5, 19 alebo 23 %. Výpočty používajú celé jednotky a centy bez binárnych desatinných čísel; základ a DPH sa zaokrúhľujú po položkách. Daňový režim zahraničného plnenia sa neurčuje automaticky.

## QR platby

- **PAY by square**: slovenský formát s kontrolou IBAN, sumy a referencie. Dátum vykonania platby zostáva prázdny, aby sa dal kód použiť aj po splatnosti.
- **QR Platba / SPAYD**: český formát. Banka zákazníka musí podporovať EUR a slovenský IBAN; všeobecná česká podpora sa týka domácich CZK platieb. Limit sumy je 9 999 999,99 EUR.
- **SEPA QR / EPC**: EUR platby v bankových aplikáciách podporujúcich EPC. QR obsahuje explicitné označenie UTF-8.

Podpora vo všetkých bankách EÚ, Revolute a Wise nie je potvrdená. Faktúra preto obsahuje aj čitateľný IBAN, sumu, menu a referenciu. QR vzniká lokálne bez odoslania údajov do cudzej služby.

Špecifikácie: [PAY by square](https://portal.bysquare.com/files/bysquare-PAYspecifications-1.2.0.pdf), [QR Platba](https://qr-platba.cz/pro-vyvojare/specifikace-formatu/), [EPC](https://www.europeanpaymentscouncil.eu/document-library/guidance-documents/quick-response-code-guidelines-enable-data-capture-initiation-sepa).

## E-maily a SMTP

V **Moja firma → Odosielanie e-mailov** nastav host, port, STARTTLS alebo SSL/TLS, prihlasovacie údaje, odosielateľa a prípadne Reply-To. Voľba Bez TLS slúži napríklad pre lokálny Mailpit. TLS certifikáty sa overujú a kontrola sa nedá vypnúť v rozhraní. OAuth nie je súčasťou tejto verzie.

Prázdne heslo ponechá uložené heslo. Pri zmene hosta alebo používateľa ho treba zadať znovu; uložené heslo môžeš aj odstrániť. Po uložení je dostupný skúšobný e-mail.

**Poslať e-mailom** na vystavenej faktúre otvorí príjemcu, predmet, vlastný text a náhľad platobných údajov. PDF je skutočná príloha, QR sú vložené PNG cez Content-ID a správa má aj textovú alternatívu. Príjemca nepotrebuje prístup k tomuto počítaču ani vzdialené obrázky.

Pod faktúrou je história posledných desiatich pokusov. „SMTP server prijal správu“ potvrdzuje prijatie SMTP serverom, nie doručenie alebo prečítanie. Opakovaná ani súbežná požiadavka s rovnakým kľúčom neodošle správu druhýkrát. Pri nepotvrdenom výsledku skontroluj server a schránku pred novým pokusom. História uchováva príjemcu, predmet, čas, verziu faktúry a odtlačok PDF; odoslanie nemení stav ani číselný rad faktúry.

## Dáta a zálohy

Aktívna databáza zostáva v pomenovanom Docker volume **`fakturacie_data`**, na ceste **`/var/www/html/data/fakturacie.sqlite`**. Táto cesta sa zachovala kvôli existujúcim inštaláciám. Obsahuje zákazníkov, nastavenia, koncepty, uložené PDF a históriu odoslaní. Prežije reštart aj nový build. `docker compose down -v` odstráni volume; na bežné zastavenie použi `stop` alebo `down` bez `-v`.

Prechod z pôvodnej PHP verzie nemení schému ani neprepisuje uložené údaje. Existujúce PDF sa čítajú ako pôvodné bajty vrátane starších importovaných súborov. Nové PDF a PDF po úprave faktúry vytvára Chromium.

Súbor `data/fakturacie.sqlite` na Macu, ak existuje, nie je aktívna databáza Docker aplikácie. Konzistentnú zálohu aktívnej databázy do hostiteľského `data/backups` vytvoríš za behu:

```sh
./bin/docker-backup.sh
```

Obnova na nový počítač alebo do **prázdneho** úložiska:

```sh
docker compose build
docker compose run --rm -T app fakturacie database import < cesta-k-zalohe.sqlite
docker compose up -d --wait
```

Import pred prvým spustením overí integritu a odmietne prepísať existujúcu databázu. Pri obnove existujúcej inštalácie najprv zálohuj aktuálne dáta; pôvodný volume nemaž bez overenej zálohy.

SMTP heslo je v privátnej databáze a jej zálohách, nevracia sa do prehliadača ani do aplikačných logov. Táto verzia ho nešifruje externým kľúčom. Záloha preto obsahuje aj prihlasovacie údaje. `.gitignore` a Docker build kontext vylučujú `.env`, databázy, PDF, e-maily, zálohy, logy a testovacie kľúče.

## Overenie

Rust testy a statické kontroly bežia pri zostavení samostatného testovacieho obrazu bez pracovného volume:

```sh
docker build --target test -t fakturacie:test .
```

Zahŕňajú presné výpočty, validáciu, súbežné číslovanie, verzie, úpravy a mazanie oboch stavov, zachovanie údajov, kópie, archiváciu zákazníkov, kompatibilitu starých PDF a e-mailových kľúčov, PAY payload a viacstranové PDF. Build spustí aj Clippy s `-D warnings` a `cargo fmt --check`.

HTTP a Mailpit skúšky používajú oddelený projekt a port 8874. Nasledujúce Node príkazy sú iba vývojové testy; pracovná aplikácia Node na hostiteľovi nepoužíva.

```sh
docker build --target runtime -t fakturacie:email-test .
FAKTURACIE_PORT=8874 docker compose -p fakturacie-email-verify -f compose.yaml -f docker/compose.mailtest.yaml up -d --wait
mkdir -p temp/verification
node --test tests/money.test.mjs
node tests/http.mjs
node tests/email-http.mjs
```

Testy vytvárajú syntetické dáta, overujú HTTP ochrany, prílohu, vložené QR, súbežné odoslanie, históriu a zlyhanie spojenia. `tests/smtp-security.mjs` navyše overuje STARTTLS, implicitné TLS, nesprávne heslo a nedôveryhodný certifikát proti Mailpitu v profile `security-test`. Testovacia CA patrí iba do dočasného testovacieho kontajnera, nikdy do pracovného obrazu.

Na nezávislé kontroly PDF/QR/MIME slúži Docker target **verify** s Popplerom a zbarom a skript `tests/verify-artifacts.py`. Test target vytvára syntetické PDF a QR v `/build/test-artifacts`; e-mailové testy ukladajú zachytené MIME a pôvodnú prílohu do `temp/verification`.

WebAmulet, eFaktúra, párovanie úhrad a verejné nasadenie zostávajú na ďalšiu etapu.
