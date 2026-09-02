# tdee-tracker

A Rust CLI that records daily weight and calorie intake, computes adaptive TDEE
from the observed weight trend, and recommends a daily calorie target to reach a
goal weight at a configured loss rate.

## Usage

```
tdee init  --start-weight 200 --goal 180 --height-cm 180 --age 35 --sex male [--rate 1.0] [--seed-tdee N] [--start-date YYYY-MM-DD]
tdee log   [--date YYYY-MM-DD] [--weight 199.6] [--kcal 1900] [--note "..."]
tdee status [--json] [--brief]
tdee history [--days 30] [--json]
tdee config [--goal 180] [--rate 1.0] [--seed-tdee N] [--window 28]
```

## Key setup

```
age-keygen -o ~/.hermes/secrets/tdee-age-key.txt
chmod 600 ~/.hermes/secrets/tdee-age-key.txt
```

Or set `TDEE_KEY_FILE` to point to your key.

## Build

Open in the dev container, then:

```
cargo build --release
cp target/release/tdee ~/.cargo/bin/tdee
```
