# Getting a Riot API key (for TFT ground truth)

The coach can use Riot's official API to fetch the final board of any TFT match: every player's
units, stars and items. Paired with captures or renders, that's exact ground truth for checking and
training unit recognition. The key is personal: never share it or commit it.

## 1. Development key (instant, expires every 24 h)

1. Open <https://developer.riotgames.com> and **Sign in** with your normal Riot account.
2. Accept the developer terms when asked.
3. The dashboard shows **Development API Key** (`RGAPI-…`). Press **Regenerate API Key** if it
   says expired. It lasts 24 hours; regenerate it the next day.
4. Limits: 20 requests per second, 100 per 2 minutes. Enough for a few hundred matches a day.

## 2. Personal key (doesn't expire, needs approval)

For long-term use without regenerating every day:

1. On the developer portal go to **Register Product → Personal API Key**.
2. Name it (e.g. "lol-coach personal"), describe it as a personal tool that reads your own and
   sampled TFT matches for analysis, not public.
3. Wait for approval (days to a few weeks). Same rate limits as the dev key.

## 3. Give it to the coach

Not wired up yet. The plan is an environment variable read by the data tools:

```bash
setx RIOT_API_KEY RGAPI-xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
```

(Open a new terminal afterwards. For the dev key, run it again each day.)

The win-probability tools (`tools/winprob/`) also read the key from `riot-api-key.txt` at the repo
root (the key alone on one line; the file is git-ignored), or from `--key-file`.

## 4. Check it works

Europe routing (EUW/EUNE accounts use `europe`; NA `americas`, KR `asia`). Replace the name and tag:

```bash
curl -H "X-Riot-Token: $RIOT_API_KEY" "https://europe.api.riotgames.com/riot/account/v1/accounts/by-riot-id/vandboabe/cox"
```

That returns your `puuid`. Then your recent TFT matches and one match's boards:

```bash
curl -H "X-Riot-Token: $RIOT_API_KEY" "https://europe.api.riotgames.com/tft/match/v1/matches/by-puuid/<puuid>/ids?count=20"
```

```bash
curl -H "X-Riot-Token: $RIOT_API_KEY" "https://europe.api.riotgames.com/tft/match/v1/matches/<matchId>"
```

In the match JSON, `info.participants[].units[]` has `character_id` (e.g. `TFT18_Ahri`), `tier`
(stars) and `itemNames`; `placement`, `level` and `traits` are there too. A `401`/`403` means the
key expired or is mistyped; `429` means slow down.
