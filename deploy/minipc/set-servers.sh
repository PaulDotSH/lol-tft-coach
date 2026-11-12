#!/usr/bin/env bash
# Sets which servers the service crawls, and removes the games and the crawl state of every
# server that is no longer named (fits read all the games on disk, so a dropped server's games
# must go). One server for each region.
#   ssh -t minipc@192.168.0.226 sudo bash /home/minipc/lol-coach/src/deploy/minipc/set-servers.sh euw1/europe kr/asia
set -euo pipefail

CONF="${CONF:-/etc/lol-coach/server.toml}"
DATA="${DATA:-/var/lib/lol-coach/winprob}"
SYSTEMCTL="${SYSTEMCTL:-systemctl}"
STAMP="$(date +%Y-%m-%d-%H%M)"

[ "$(id -u)" -eq 0 ] || [ -n "${NOT_ROOT_OK:-}" ] || { echo "run with sudo"; exit 1; }
[ $# -ge 1 ] || { echo "name the servers to keep, as platform/region: euw1/europe kr/asia"; exit 1; }
keep=()
for s in "$@"; do
  [[ "$s" =~ ^[a-z0-9]+/[a-z]+$ ]] || { echo "not platform/region: $s"; exit 1; }
  keep+=("${s%%/*}")
done
grep -q '^servers = ' "$CONF" || { echo "no servers line in $CONF"; exit 1; }
list=$(printf '"%s", ' "$@")
list="[${list%, }]"

cp -p "$CONF" "$CONF.bak-$STAMP-servers"
sed -i -E "s|^servers = .*|servers = $list|" "$CONF"
echo "servers = $list"

$SYSTEMCTL stop lol-coach-server.service
shopt -s nullglob
for state in "$DATA"/*/*.json; do
  platform="$(basename "$state" .json)"
  [ "$platform" = fit ] && continue
  for k in "${keep[@]}"; do [ "$k" = "$platform" ] && continue 2; done
  patch_dir="$(dirname "$state")"
  n=$(find "$patch_dir/raw" -maxdepth 1 -name "${platform^^}_*.match.json.gz" 2>/dev/null | wc -l)
  find "$patch_dir/raw" -maxdepth 1 -name "${platform^^}_*.json.gz" -delete 2>/dev/null || true
  rm -f "$state" "$patch_dir/rows.npz"
  echo "removed $platform from $(basename "$patch_dir"): $n games and its crawl state"
done
$SYSTEMCTL start lol-coach-server.service
sleep 2
$SYSTEMCTL is-active lol-coach-server.service
