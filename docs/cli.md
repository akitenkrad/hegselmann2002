**English** | [日本語](cli.ja.md)

# CLI

The Rust binary `hegselmann-bc` (run via `cargo run --release -- …`) exposes two subcommands: `run` and `sweep`.

## `run` — single simulation

Run the symmetric BC dynamics for one `(n, ε)` pair.

```bash
cargo run --release -- run \
    --n 625 --eps 0.15 --start uniform \
    --max-iterations 100 --tol 1e-6 --seed 42
```

| Flag | Default | Description |
|---|---|---|
| `--n` | 625 | number of agents `n` |
| `--eps` | 0.15 | symmetric confidence radius `ε` (used when neither `--eps-l` nor `--eps-r` is given; sets `ε_l = ε_r = ε`) |
| `--eps-l` | none | left confidence radius `ε_l` for the asymmetric BC mode (paper §4.2 / Fig. 10–13). Must be paired with `--eps-r`; if omitted, the symmetric `--eps` is used. |
| `--eps-r` | none | right confidence radius `ε_r` for the asymmetric BC mode. Must be paired with `--eps-l`. |
| `--start` | uniform | initial opinion profile (`uniform` \| `regular`) |
| `--max-iterations` | 100 | maximum number of steps `T` |
| `--tol` | 1e-6 | convergence tolerance (stop when `max|Δx| < tol`) |
| `--seed` | random | RNG seed (omit for a randomly drawn seed) |
| `--output-dir` | results | output base directory |

**Output files:**

Each execution is stored as a runvault run directory. The run directory *is* the output location, so no timestamped directory and no `latest` symlink are created here. Ask `runvault` for the path of the most recent finished run.

```bash
runvault path --experiment hegselmann-bc --latest --subcommand run --standalone
```

```
results/
└── hegselmann-bc/                                  ← experiment
    ├── latest_finished -> run_20260831_145450_...  ← the last run that finished
    ├── run_20260831_145450_c3b078ae_298d/          ← <subcommand>_<time>_<cfg8>_<exec4>
    │   ├── run.json                                ← metadata (git commit / environment / paper)
    │   ├── config.json                             ← an envelope; the conditions sit under ["parameters"]
    │   ├── metrics.csv                             ← long form (step / scope / name / value)
    │   ├── events.jsonl                            ← per-step observations + the terminal row (phase label)
    │   ├── status.json                             ← how it ended and how long it took
    │   ├── manifest.csv                            ← hashes of artifacts/ and logs/
    │   └── artifacts/
    │       └── opinions.csv                        ← long-format opinion trajectory: t, agent_id, opinion
    └── figures/                                    ← what the plotting scripts write (outside the run)
        └── run_20260831_145450_c3b078ae_298d/
```

Figures are drawn after the run has ended, so they go **outside** the run directory (`<experiment>/figures/<run_slug>/`). `manifest.csv` is settled by `finish()`, so a file added to `artifacts/` afterwards would carry no hash.

`metrics.csv` is long form, one value per row. The five per-step metrics `n_surviving` / `mean` / `variance` / `n_splits` / `max_delta` carry a `step`; `converged` (0.0 / 1.0) and `final_iteration`, which describe the whole run with one number each, sit at `scope=run` with no `step`.

**The phase is not a metric.** consensus / polarization / plurality is a label rather than a number, and it follows uniquely from `n_surviving` (≤1 / 2–10 / >10) — assigning it a number would add nothing. The final phase is therefore kept as a label on the `terminal` row of `events.jsonl` (`"phase": "polarization"`); the same row carries convergence and censoring through `outcome` / `censored` / `budget`. See [`show-experiment-settings`](visualization.md#show-experiment-settings) for displaying the conditions.

### Example phase outcomes (n = 625, uniform)

```bash
cargo run --release -- run --n 625 --eps 0.05 --seed 42   # → plurality (many clusters)
cargo run --release -- run --n 625 --eps 0.15 --seed 42   # → polarization (two camps)
cargo run --release -- run --n 625 --eps 0.25 --seed 42   # → consensus (one camp)
```

### Asymmetric BC example (paper §4.2 / Fig. 10–13)

```bash
# Asymmetric BC (paper Fig. 11 style: wider right → final mean drifts to the right)
cargo run --release -- run --n 625 --eps-l 0.05 --eps-r 0.25 --start uniform --seed 42
```

## `sweep` — ε sweep

Sweep ε across a range and aggregate per-run final metrics for analysis.

```bash
cargo run --release -- sweep \
    --eps-min 0.01 --eps-max 0.40 --eps-step 0.01 \
    --n 625 --start uniform --runs 50 --seed 42
```

| Flag | Default | Description |
|---|---|---|
| `--eps-min` | 0.01 | minimum ε |
| `--eps-max` | 0.40 | maximum ε (inclusive) |
| `--eps-step` | 0.01 | ε step |
| `--n` | 625 | number of agents |
| `--start` | uniform | initial opinion profile (`uniform` \| `regular`) |
| `--runs` | 50 | independent trials per ε |
| `--max-iterations` | 100 | maximum steps |
| `--tol` | 1e-6 | convergence tolerance |
| `--seed` | 42 | seed base (each trial derives an independent seed) |
| `--output-dir` | results | output base directory |

Each trial derives an independent seed via `derive_seed(seed, &[eps.bits, run_id])`, so trials are reproducible and uncorrelated. The sweep is run sequentially (no `rayon`) for simplicity.

**Output files:**

A sweep is recorded as one parent run plus one child run per ε. The children do not sit under the parent but beside it in the experiment directory, pointing back through `lineage.parent_run_uid`. No one-row-per-trial summary CSV is written — the same values are in each child's `events.jsonl`.

The children's subcommand is `sweep-point`, not `run`: a `run` is a single simulation while a child is `runs` of them under one condition, and letting two different shapes share a name would make `runvault path --subcommand run` ambiguous.

```
results/
└── hegselmann-bc/
    ├── sweep_20260831_145451_23b44915_976d/         ← parent; parameters hold the ε grid
    │   ├── run.json                                 ← carries lineage.sweep_id; rng.master_seed is null
    │   └── config.json
    ├── sweep-point_20260831_145451_2c10e235_bfc8/   ← child (one ε); lineage.parent_run_uid = the parent
    │   ├── config.json                              ← parameters carry eps_l / eps_r / runs
    │   ├── metrics.csv                              ← only the condition's aggregates (scope=run)
    │   └── events.jsonl                             ← one observation + one terminal row per trial
    └── ...
```

A child's `terminal` rows are what the old `sweep_summary.csv` held: `unit_id` (`trial-<i>`) / `seed` / `t` (= `final_iteration`) / `censored` (the negation of convergence) / `n_surviving` / `mean` / `variance` / `n_splits` / `max_delta` / `phase`. Per-trial values cannot go into `metrics.csv` — (`run_uid`, `step`, `scope`, `name`) would repeat — so a plot that needs the spread rebuilds it from here.

The parent's path comes from `runvault path --experiment hegselmann-bc --latest --subcommand sweep`. `hegselmann-bc-tools visualize-sweep` takes that parent and reassembles the familiar summary table from its children.

## Phase 3 status

Asymmetric BC (`--eps-l` / `--eps-r`; paper §4.2 / Fig. 10–13) is **supported** via the upstream `HegselmannKrauseMechanism::with_asymmetric` (socsim-mechanisms PR #47); no custom mechanism is required. The remaining Phase 3 item is the batched paper-figure reproduction (`reproduce` subcommand), which is not yet implemented.
