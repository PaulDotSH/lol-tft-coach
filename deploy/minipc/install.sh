#!/usr/bin/env bash
# Root steps for the LoL Coach service on the minipc. Run once, and again after a new build:
#   ssh -t minipc@192.168.0.226 sudo bash /home/minipc/lol-coach/src/deploy/minipc/install.sh
# Safe to run again: it replaces the binary, the tools and the unit, and leaves an existing
# config, token, Caddy block, DNS record and backup group alone. It prints no secret.
set -euo pipefail

OWNER_HOME="${OWNER_HOME:-/home/minipc}"
SRC="$OWNER_HOME/lol-coach/src"
HERE="$SRC/deploy/minipc"
BIN="$SRC/target/production/lol-coach-server"
HOST="lol.foarte.top"
PORT=8787
ETC=/etc/lol-coach
DATA=/var/lib/lol-coach
OPT=/opt/lol-coach
CADDYFILE=/etc/caddy/Caddyfile
DDNS=/usr/local/bin/cf-ddns.py
BACKUP="$OWNER_HOME/.local/bin/stack-backup.sh"
STAMP="$(date +%Y-%m-%d-%H%M)"

[ "$(id -u)" -eq 0 ] || { echo "run with sudo"; exit 1; }
[ -x "$BIN" ] || { echo "no build at $BIN: build first (see README.md here)"; exit 1; }

echo "== binary and fit tools =="
install -D -m 755 "$BIN" "$OPT/bin/lol-coach-server.new"
mv "$OPT/bin/lol-coach-server.new" "$OPT/bin/lol-coach-server"
install -d -m 755 "$OPT/tools/winprob"
install -m 644 "$SRC/tools/winprob/features.py" "$SRC/tools/winprob/fit.py" "$SRC/tools/winprob/riot_format.py" "$OPT/tools/winprob/"

echo "== directories =="
install -d -m 750 -o caddy -g caddy "$DATA"
install -d -m 750 -o root -g caddy "$ETC"

echo "== Riot key =="
if [ -f "$OWNER_HOME/lol-coach/riot-api-key.txt" ]; then
  install -m 640 -o root -g caddy "$OWNER_HOME/lol-coach/riot-api-key.txt" "$ETC/riot-api-key.txt"
  shred -u "$OWNER_HOME/lol-coach/riot-api-key.txt" 2>/dev/null || rm -f "$OWNER_HOME/lol-coach/riot-api-key.txt"
  echo "  moved into $ETC"
elif [ -f "$ETC/riot-api-key.txt" ]; then
  echo "  already in $ETC"
else
  echo "  none: lookups and the crawl stay off until $ETC/riot-api-key.txt exists (640 root:caddy)"
fi

echo "== config =="
if [ -f "$ETC/server.toml" ]; then
  echo "  $ETC/server.toml kept as it is"
else
  umask 027
  cat > "$ETC/server.toml" <<EOF
listen = "127.0.0.1:$PORT"
data_dir = "$DATA"
tokens = ["$(openssl rand -hex 32)"]
riot_key_file = "$ETC/riot-api-key.txt"
lookup_ttl_hours = 12
max_file_mb = 32

[crawl]
servers = ["euw1/europe", "kr/asia"]
max_games = 100000
fit_at = 10000
refit_every = 2500
python = "/usr/bin/python3"
tools_dir = "$OPT/tools/winprob"
EOF
  chown root:caddy "$ETC/server.toml"
  chmod 640 "$ETC/server.toml"
  echo "  written with a new token (read it with: sudo grep tokens $ETC/server.toml)"
fi

echo "== games crawled on a PC =="
IMPORT="$OWNER_HOME/lol-coach/import/winprob"
if [ -d "$IMPORT" ]; then
  systemctl stop lol-coach-server.service 2>/dev/null || true
  install -d -m 750 -o caddy -g caddy "$DATA/winprob"
  cp -an "$IMPORT/." "$DATA/winprob/"
  find "$DATA/winprob" -name raw.txt -delete
  chown -R caddy:caddy "$DATA/winprob"
  rm -rf "$OWNER_HOME/lol-coach/import"
  echo "  moved into $DATA/winprob: $(find "$DATA/winprob" -name '*.match.json.gz' | wc -l) games there now"
else
  echo "  none waiting"
fi

echo "== service =="
install -m 644 "$HERE/lol-coach-server.service" /etc/systemd/system/lol-coach-server.service
systemctl daemon-reload
systemctl enable lol-coach-server.service >/dev/null
systemctl restart lol-coach-server.service
sleep 2
if curl -fsS "http://127.0.0.1:$PORT/v1/health" >/dev/null; then
  echo "  answers on 127.0.0.1:$PORT"
else
  echo "  NOT answering: journalctl -u lol-coach-server -n 30"
  exit 1
fi

echo "== Caddy =="
if grep -q "^$HOST {" "$CADDYFILE"; then
  echo "  $HOST is already in $CADDYFILE"
else
  cp -p "$CADDYFILE" "$CADDYFILE.bak-$STAMP-lol"
  cat "$HERE/Caddyfile.block" >> "$CADDYFILE"
  if caddy validate --config "$CADDYFILE" --adapter caddyfile >/dev/null 2>&1; then
    systemctl reload caddy
    echo "  block added, Caddy reloaded (backup: $CADDYFILE.bak-$STAMP-lol)"
  else
    cp -p "$CADDYFILE.bak-$STAMP-lol" "$CADDYFILE"
    echo "  Caddy refused the block: $CADDYFILE put back, nothing reloaded"
    caddy validate --config "$CADDYFILE.bak-$STAMP-lol" --adapter caddyfile >/dev/null 2>&1 || echo "  (the file before this run does not validate either)"
    exit 1
  fi
fi

echo "== DNS =="
if grep -q "\"$HOST\"" "$DDNS"; then
  echo "  $HOST is already in $DDNS"
else
  cp -p "$DDNS" "$DDNS.bak-$STAMP-lol"
  sed -i -E "/^RECORDS = \[/ s/\]\s*$/, \"$HOST\"]/" "$DDNS"
  if grep -q "\"$HOST\"" "$DDNS" && python3 -m py_compile "$DDNS"; then
    systemctl start cloudflare-ddns.service && echo "  record added and the updater run once" || echo "  record added; the updater failed: journalctl -u cloudflare-ddns -n 20"
  else
    cp -p "$DDNS.bak-$STAMP-lol" "$DDNS"
    echo "  could not add $HOST to RECORDS: $DDNS put back; add it by hand"
  fi
fi

echo "== backup group =="
if grep -q "lol-coach" "$BACKUP"; then
  echo "  already in $BACKUP"
else
  cp -p "$BACKUP" "$BACKUP.bak-$STAMP-lol"
  if python3 - "$BACKUP" <<'EOF' && bash -n "$BACKUP"; then
import sys
path = sys.argv[1]
text = open(path, encoding="utf-8").read()
anchor = '  chmod -R go-rwx "$B/databases/ai-router" "$B/configs/ai-router" 2>/dev/null || true\n}\n'
group = '''  chmod -R go-rwx "$B/databases/ai-router" "$B/configs/ai-router" 2>/dev/null || true

  echo "== lol-coach (player data + config; match data and the Riot key deliberately NOT copied) =="
  mkdir -p "$B/databases/lol-coach" "$B/configs/lol-coach"
  if [ -e /var/lib/lol-coach/players ]; then
    rsync -a --delete /var/lib/lol-coach/players/ "$B/databases/lol-coach/players/" \\
      && echo "  lol-coach players: $(find "$B/databases/lol-coach/players" -type f | wc -l) file(s)"
  else
    echo "  (skip lol-coach players — not found)"
  fi
  copy_cfg "$B/configs/lol-coach" /etc/lol-coach/server.toml /etc/systemd/system/lol-coach-server.service
  chmod -R go-rwx "$B/databases/lol-coach" "$B/configs/lol-coach" 2>/dev/null || true
}
'''
if text.count(anchor) != 1:
    sys.exit("anchor not found")
open(path, "w", encoding="utf-8").write(text.replace(anchor, group))
EOF
    chown --reference="$BACKUP.bak-$STAMP-lol" "$BACKUP"
    echo "  group added to the root-only part (backup: $BACKUP.bak-$STAMP-lol)"
  else
    cp -p "$BACKUP.bak-$STAMP-lol" "$BACKUP"
    echo "  could not add the group: $BACKUP put back; add it by hand"
  fi
fi

echo
echo "done. Check from outside once DNS has the record: curl -s https://$HOST/v1/health"
