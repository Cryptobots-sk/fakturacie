#!/bin/zsh
set -eu
cd "${0:A:h}"
if ! docker info >/dev/null 2>&1; then
  print 'Najprv spusti Docker Desktop a počkaj, kým bude pripravený.'
  read '?Stlač Enter na zatvorenie.'
  exit 1
fi
docker compose up -d --build --wait
print 'Fakturácie bežia v Dockeri: http://127.0.0.1:8873'
print 'Toto okno môžeš zatvoriť. Zastavenie: docker compose stop'
