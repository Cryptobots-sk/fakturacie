# Fakturácie
Osobná lokálna fakturácia pre Cryptobots. Slovenské rozhranie, EUR, žiadne platené vyhľadávanie firiem.

Schválený rozsah: vlastný adresár zákazníkov, editor priamo na faktúre, automatické ukladanie konceptov, číslovanie pri vystavení, PDF s lokálne generovaným platobným QR, úprava a vymazanie každej faktúry bez ohľadu na stav. Výber najviac dvoch z PAY by square, SPAYD a EPC. Nová inštalácia začína bez zákazníkov a faktúr; existujúce dáta možno obnoviť z privátnej zálohy.

Hlavný tok: Nová faktúra → vybrať zákazníka → dopísať položky a dátumy → skontrolovať sumu → vystaviť a stiahnuť PDF. Fakturačný papier je pracovná plocha; obyčajné formuláre slúžia iba adresáru a nastaveniam firmy.

Vzhľad: pokojný, čitateľný pracovný nástroj, svetlé sivé pozadie, biely dokument, tmavá modrá pre dôležité akcie, bez dekoratívnych grafov. Klávesnica, viditeľný fokus, skutočné popisy polí, použiteľnosť od 320 px.

WebAmulet, bankové párovanie a verejné nasadenie s prihlásením sú mimo tejto verzie. QR podpora konkrétnych bánk sa nesľubuje bez overenia.

Úprava vystavenej faktúry sa potvrdzuje ručne, zachová číslo aj stav a obnoví PDF aj QR v jednej transakcii. Vymazanie vyžaduje potvrdenie a neznižuje číselný rad.

E-mail: vlastné SMTP nastavenia, skúšobný e-mail, príjemca/predmet/správa s náhľadom, PDF príloha, platobné údaje a vložené QR, história prijatia SMTP a ochrana opakovanej požiadavky. Overenie lokálnym Mailpitom; produkčné doručenie závisí od nakonfigurovaného SMTP.
