#!/usr/bin/env bash
# Sets how many games the crawl keeps (max_games, over the two newest patches), installs the
# unit file again (its memory limit is sized for the fit tools at that many games) and restarts
# the service.
#   ssh -t minipc@192.168.0.226 sudo bash /home/minipc/lol-coach/src/deploy/minipc/set-max-games.sh 100000
set -euo pipefail

CONF="${CONF:-/etc/lol-coach/server.toml}"
UNIT="${UNIT:-/etc/systemd/system/lol-coach-server.service}"
SYSTEMCTL="${SYSTEMCTL:-systemctl}"
HERE="$(cd "$(dirname "$0")" && pwd)"
STAMP="$(date +%Y-%m-%d-%H%M)"

[ "$(id -u)" -eq 0 ] || [ -n "${NOT_ROOT_OK:-}" ] || { echo "run with sudo"; exit 1; }
[[ "${1:-}" =~ ^[1-9][0-9]*$ ]] || { echo "name the number of games: 100000"; exit 1; }
grep -q '^max_games = ' "$CONF" || { echo "no max_games line in $CONF"; exit 1; }

cp -p "$CONF" "$CONF.bak-$STAMP-games"
sed -i -E "s|^max_games = .*|max_games = $1|" "$CONF"
grep '^max_games = ' "$CONF"

install -m 644 "$HERE/lol-coach-server.service" "$UNIT"
grep '^MemoryMax=' "$UNIT"
$SYSTEMCTL daemon-reload
$SYSTEMCTL restart lol-coach-server.service
sleep 2
$SYSTEMCTL is-active lol-coach-server.service
