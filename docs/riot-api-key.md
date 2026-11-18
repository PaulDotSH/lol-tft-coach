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

## 5. Production key (to do, for the win-probability content and public calculator)

Noted 2026-11-12, not applied for yet. A personal key covers the fit in `tools/winprob` (private
use, no site verification). Anything open to the public, such as a web calculator, needs a
production key: "You may not run your application for public consumption using a personal key."

Before applying:

1. Get the domain working.
2. Put up a simple site that explains the app (what it does, what data it reads, that the fitted
   table holds no player data). Riot wants a working prototype and says an unfinished website is
   unlikely to be approved.
3. Apply on the developer portal (**Register Product → Production API Key**) with the description
   below and the site's URL.
4. Verify the domain: save the string the portal gives as `riot.txt` at the site's root, reachable
   at `https://<domain>/riot.txt`. No production key is granted without a verified website.
5. Wait for review (about two weeks, up to a month).

Limits start at 500 requests per 10 s and 30,000 per 10 min per region. One product per key.

### Draft description

> I want to start making content for league of legends, besides this I have developed a small
> project used by me and my friends to help decide builds, give game recaps, and tips for next
> games based on current performance.
>
> This would be used from a server of mine not personal machines. Reading league v4 ranked ladder,
> the recent match ids of those players (match-v5), and each match with its timeline. From the
> timelines it builds per-minute team totals (item gold, levels, CS, kills, towers, dragons, Baron)
> and fits a logistic model of which team won from that state. The result is a small table of
> coefficients.
>
> My desktop app uses that table to show me an estimated win chance from the scoreboard state in
> my own games and in my post-game review. The fitted table contains no player data.
>
> Content ideas I have:
> - What each stat is worth, minute by minute
> - How big a gold lead do you need to be handed a "definite" win
> - Most important objectives depending on ELO
> - A small calculator to check our model, with multiple input values and a predictor
> - Can we create a win probability calculator like DOTA for LoL esports?

To settle before sending: say outright that there is no betting or gambling (a win-probability
tool next to "esports" reads that way, and the policy bans it); match-v5 has no pro games, so the
esports idea needs another data source or should be dropped from the request; "depending on ELO"
means by ranked tier, not an MMR or ELO estimate (those are prohibited).
