# flock-denver

Map of automated license plate readers (Flock Safety and others) in the Denver metro.

- `fetcher/` — Rust CLI. Queries the OpenStreetMap Overpass API for
  `man_made=surveillance` nodes tagged as ALPR / Flock inside a bbox and writes
  `data/cameras.json`. Same crowdsourced dataset DeFlock uses (ODbL).
- `server/` — Node/Express. Serves the Leaflet map UI and `/api/cameras`;
  `POST /api/refresh` shells out to the Rust fetcher.

## Run

```sh
# 1. pull data (needs cargo; first build takes a minute)
cd fetcher && cargo run --release            # or: cargo run --release -- --flock-only
# custom area: cargo run --release -- --bbox 39.6,-105.2,39.9,-104.8

# 2. serve
cd ../server && npm install && npm start     # http://localhost:3000
```

Overpass rate-limits — if you get a 429/504, wait a minute and retry.
Coverage is only as good as what volunteers have mapped; if you spot a camera
that's missing, add it on openstreetmap.org and re-pull.
