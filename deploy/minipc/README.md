# LoL Coach service on the minipc (lol.foarte.top)

Design: `docs/superpowers/specs/2026-11-12-minipc-service-design.md`. The minipc is
`minipc@192.168.0.226` (SSH key). Root steps are one script that the owner runs; everything
else needs no root.

## From the PC, no root

Send the committed source and build it there (the build tunes for that CPU, so it is built on
the minipc):

```
git -c core.autocrlf=false archive --format=tar.gz HEAD | ssh minipc@192.168.0.226 "mkdir -p ~/lol-coach/src && tar -xz -C ~/lol-coach/src"
```

```
ssh minipc@192.168.0.226 "cd ~/lol-coach/src && RUSTFLAGS='-C target-cpu=native' nice cargo build --profile production -p lol-coach-server -j 6"
```

The Riot key, once (the install script moves it into `/etc/lol-coach` and removes this copy):

```
scp riot-api-key.txt minipc@192.168.0.226:lol-coach/riot-api-key.txt
```

Games already crawled on the PC with `tools/winprob/crawl.py` (same key), once, so the minipc
goes on from them. Stop the PC crawl first; only before the service has crawled that server on
that patch:

```
py -3.12 -I deploy/minipc/seed.py %LOCALAPPDATA%\lol-coach\dev\winprob\16.22 seed-out
```

then send the state file and the raw files to `~/lol-coach/import/winprob` on the minipc (state
file beside a `raw/` directory, as under the service's `winprob/`; from the PC's `raw` folder:
`tar -cf - . | ssh minipc@192.168.0.226 "tar -xf - -C ~/lol-coach/import/winprob/16.22/raw"`).
The install script moves them into `/var/lib/lol-coach/winprob` before it starts the service.

## Root, by the owner

```
ssh -t minipc@192.168.0.226 sudo bash /home/minipc/lol-coach/src/deploy/minipc/install.sh
```

It installs the binary and the fit tools in `/opt/lol-coach`, writes `/etc/lol-coach/server.toml`
with a new token when there is none, installs and restarts `lol-coach-server.service`
(`User=caddy`, `127.0.0.1:8787`), and, each only when it is not there yet: the `lol.foarte.top`
block in `/etc/caddy/Caddyfile`, the name in `/usr/local/bin/cf-ddns.py`, and a `lol-coach` group
in `~/.local/bin/stack-backup.sh` (player data and config; not the match data, which can be
crawled again, and not the Riot key). Each file it edits gets a `.bak-<date>-lol` copy first,
and a Caddyfile that does not validate is put back. Run it again after every new build.

## Which servers

```
ssh -t minipc@192.168.0.226 sudo bash /home/minipc/lol-coach/src/deploy/minipc/set-servers.sh euw1/europe kr/asia
```

sets the crawl's servers in the config, removes the games and the crawl state of every server
no longer named, and restarts the service.

## How many games

```
ssh -t minipc@192.168.0.226 sudo bash /home/minipc/lol-coach/src/deploy/minipc/set-max-games.sh 100000
```

sets `max_games` in the config, installs the unit file again (its memory limit is sized for the
fit tools at that many games) and restarts the service. A lower number deletes nothing by itself:
the crawl stops until a new patch makes room.

## Check

```
curl -s https://lol.foarte.top/v1/health
```

The token for the app is in `/etc/lol-coach/server.toml` (`sudo grep tokens /etc/lol-coach/server.toml`).
Logs: `journalctl -u lol-coach-server -f`. Data: `/var/lib/lol-coach` (`players/` is what the
backup takes; `winprob/` is match data, at most `max_games` games, 100,000). One crawl for a region at a
time: stop a crawl on the PC before the service crawls with the same key.
