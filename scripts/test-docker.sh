#!/usr/bin/env bash
# Tests de l'image Docker : la construit et la démarre avec docker compose
# (projet séparé « paint-test », sur un port libre), vérifie le durcissement du
# conteneur, puis lance toute la suite Playwright contre lui.
# Arguments supplémentaires transmis à Playwright (ex. : tests/e2e/web.spec.js).
set -euo pipefail
cd "$(dirname "$0")/.."
free_port() { node -e "const s = require('net').createServer().listen(0, '127.0.0.1', () => { console.log(s.address().port); s.close(); })"; }
export PAINT_PORT="${PAINT_TEST_PORT:-$(free_port)}" PAINT_HOST=127.0.0.1
compose=(docker compose -p paint-test)
trap '"${compose[@]}" down >/dev/null 2>&1' EXIT
"${compose[@]}" up -d --build --wait
id="$("${compose[@]}" ps -q paint)"
check() { [ "$2" = "$3" ] || { echo "Échec : $1 ($2 au lieu de $3)" >&2; exit 1; }; }
check "utilisateur" "$(docker exec "$id" id -u)" 101
check "système de fichiers en lecture seule" "$(docker inspect -f '{{.HostConfig.ReadonlyRootfs}}' "$id")" true
check "capacités retirées" "$(docker inspect -f '{{.HostConfig.CapDrop}}' "$id")" "[ALL]"
check "état de santé" "$(docker inspect -f '{{.State.Health.Status}}' "$id")" healthy
echo "Conteneur : sans root, lecture seule, sans capacités, en bonne santé."
PAINT_URL="http://127.0.0.1:$PAINT_PORT/" npx playwright test "$@"
