# Vzhľad

Pracovná aplikácia: systémové písmo, Bootstrap 5.3 priestorová stupnica, základ 15 px / 1.55, nadpisy semibold, základná farba #17567b. Svetlé sivé pozadie a biela pracovná plocha. Žiadne marketingové karty ani dekoratívne grafy.

Dokument: spoločná šablóna `app/Presentation/templates/document/invoice.latte` a pravidlá `assets/invoice.css`, písmo DejaVu Sans, telo 9 pt, názov 25 pt, firmy 12 pt, úhrada 14 pt. A4 210 × 297 mm, okraje 17 mm po bokoch a 18 mm hore. Fyzické rozmery a bodové písmo sú zámerné kvôli zhode s exportom. Formulár používa prekryté polia; zobrazovaný text zachováva rozloženie. Tlačidlá pridania/odstránenia položky neovplyvňujú geometriu dokumentu. Na malom displeji sa celá A4 zmenší, tlačidlo Priblížiť sprístupní pôvodnú veľkosť s posúvaním.

Farby stavov `issued` a `draft` sú súčasťou Bootstrap theme-colors. Súvisiace údaje sa zoskupujú pomocou gridu, tabuľky faktúry vyjadrujú dátové vzťahy. Prvky majú menovky, viditeľný fokus a potvrdenie vystavenia so sumou a odberateľom. Stav ukladania používa živé oznámenia. QR profily majú jasné názvy a pravdivé hranice podpory bánk.
