**English** | [日本語](visualization.ja.md)

# Visualization

The Python package `hegselmann-bc-tools` (a uv workspace member) reads runvault run directories and produces figures. Install once with `uv sync` at the workspace root.

```bash
uv sync
uv run hegselmann-bc-tools visualize
uv run hegselmann-bc-tools visualize-sweep
uv run hegselmann-bc-tools show-experiment-settings
```

The unified CLI dispatches to one of three subcommands; arguments after the subcommand are passed straight to the corresponding module's argparse.

## `visualize` — opinion trajectory

Reads `artifacts/opinions.csv` and the long-form `metrics.csv` from a `run` directory and writes two figures:

- `opinion_trajectory.png` — the opinion trajectory (x = time, y = opinion in `[0,1]`, one translucent line per agent; paper Fig. 2 / 7 / 8 style). Final cluster centres are overlaid as dashed horizontal lines so plurality / polarization / consensus are visually obvious. The title shows the surviving-opinion count `n_surviving`.
- `metrics_timeseries.png` — three panels: `n_surviving` (log y), `variance`, and `max|Δx|` (log y, the convergence indicator).

```bash
uv run hegselmann-bc-tools visualize
uv run hegselmann-bc-tools visualize --results-dir "$(runvault path --experiment hegselmann-bc --latest --subcommand run --standalone)"
```

`--subcommand run --standalone` narrows the search, so neither a sweep parent nor one of its children can be picked up.

| Flag | Default | Description |
|---|---|---|
| `--results-dir` | `runvault path --latest --subcommand run --standalone` | the run directory |
| `--results-root` | results | the results root runvault searches when `--results-dir` is omitted |
| `--output-dir` | `<experiment>/figures/<run_slug>/` | figure output directory |

## `visualize-sweep` — phase diagram

Takes a sweep's parent run, gathers the `events.jsonl` of the children (`sweep-point`) that point at it through `lineage.parent_run_uid`, rebuilds the one-row-per-trial table from their terminal rows, and writes (`sweep_summary.csv` is no longer written). The individual trials are needed rather than the condition's mean, because the ±1σ band is drawn from their spread:

- `visualize_sweep.png` — a two-panel figure:
  - **Top:** mean number of surviving opinions vs ε with a ±1σ band (paper Fig. 3 / 12a style; the dashed line marks the consensus boundary at 1 cluster).
  - **Bottom:** mean final opinion vs ε with a ±1σ band (paper Fig. 12c style; for symmetric BC this should hug `x̄ = 0.5`, marked by the dashed reference line).

The numerical consensus brink `ε*` (smallest ε at which the trial-averaged surviving count first reaches 1) is also printed on stdout.

```bash
uv run hegselmann-bc-tools visualize-sweep
uv run hegselmann-bc-tools visualize-sweep --sweep-dir "$(runvault path --experiment hegselmann-bc --latest --subcommand sweep)"
```

| Flag | Default | Description |
|---|---|---|
| `--sweep-dir` | `runvault path --latest --subcommand sweep` | the sweep parent's run directory (`--results-dir` is also accepted) |
| `--results-root` | results | the results root runvault searches when `--sweep-dir` is omitted |
| `--output-dir` | `<experiment>/figures/<run_slug>/` | figure output directory |

## `show-experiment-settings`

Pretty-prints a run directory's experiment conditions from its `config.json`. That file is runvault's envelope (`schema_version` / `run_uid` / `runvault` / `parameters`) and the conditions sit under `parameters`. Whether the directory is a `run`, a `sweep` or a `sweep-point` is answered by `run.json`'s `subcommand`, so any of them can be passed. A legacy flat `config.json` / `sweep_config.json` is read too. Use `--json` for machine-readable output.

```bash
uv run hegselmann-bc-tools show-experiment-settings
uv run hegselmann-bc-tools show-experiment-settings --results-dir "$(runvault path --experiment hegselmann-bc --latest --subcommand sweep)"
uv run hegselmann-bc-tools show-experiment-settings --json
```

## `reproduce` — batch paper-figure reproduction

Runs the Rust binary (`cargo run --release -- run / sweep ...`) once per Figure spec, reads back what it produced, and writes a consolidated PNG per Figure into a single timestamped directory. The intermediate data (`artifacts/opinions.csv` / `metrics.csv` / `events.jsonl`) stays in the runvault run directory (`results/hegselmann-bc/<run_slug>/`); their paths are recorded in `reproduce_summary.json`.

Which run belongs to the current invocation is asked of `runvault path --latest` rather than guessed from a directory name or an mtime. Several specs can therefore start within the same second without colliding, and the second-boundary sleep this used to need is gone.

```bash
uv run hegselmann-bc-tools reproduce                  # full reproduction (paper values)
uv run hegselmann-bc-tools reproduce --quick          # lightweight smoke run (n=125, runs=5)
uv run hegselmann-bc-tools reproduce --specs fig02,fig03
uv run hegselmann-bc-tools reproduce --skip-build     # reuse a pre-built cargo binary
```

Supported Figure specs (paper §4 benchmarks):

| Spec | Subcommand | Parameters | Expected behaviour |
|---|---|---|---|
| `fig02` | `run` | `n=625, ε=0.01, uniform, max_iter=50, seed=42` | ≈ 38 clusters (fragmentation) |
| `fig07` | `run` | `n=100, ε=0.05, regular, max_iter=50, seed=1` | 8 splits (polarization) |
| `fig08` | `run` | `n=100, ε=0.25, regular, max_iter=30, seed=1` | consensus |
| `fig03` | `sweep` | `n=625, ε∈[0.01,0.40] step=0.01, runs=50, seed=42` | sharp drop in surviving opinions (3-phase transition) |
| `fig12` | derived from `fig03` | same sweep data | mean final opinion + final variance, 2-panel |
| `fig11` | `run` × 4 | `n=625, max_iter=100, seed=42`, asymmetric `(ε_l,ε_r)∈{(.20,.20),(.15,.25),(.10,.30),(.05,.35)}` | 2×2 panel; final mean shifts right as ε_r widens |

Output structure:

```
results/reproduce_<YYYYMMDD_HHMMSS>/
├── reproduce_summary.json        # per-spec args / cargo invocations / status / timings
└── figures/
    ├── fig02_n625_eps0.01_uniform.png
    ├── fig03_sweep_n_surviving.png
    ├── fig07_n100_eps0.05_regular.png
    ├── fig08_n100_eps0.25_regular.png
    ├── fig11_asymmetric_panel.png
    └── fig12_sweep_mean_variance.png
```

| Flag | Default | Description |
|---|---|---|
| `--specs` | (all) | comma-separated spec IDs to run (`fig02,fig03,fig07,fig08,fig11,fig12`) |
| `--output-dir` | results | result root (workspace-relative); the reproduce bundle lands at `<output-dir>/reproduce_<ts>/` |
| `--cargo-output-dir` | same as `--output-dir` | the results root passed as `--output-dir` to cargo |
| `--workspace-root` | (auto) | override the cargo workspace root (also via env `HEGSELMANN_BC_PROJECT_ROOT`) |
| `--quick` | off | shrink fig02 / fig03 / fig12 (n=125, runs=5) for a fast smoke run; do not use for paper-value verification |
| `--skip-build` | off | skip `cargo build --release` (assumes the binary is already built) |

## Note on fonts

The scripts request `font.family = "Hiragino Sans"` for Japanese labels (macOS). On other platforms, substitute an installed CJK font in the `plt.rcParams` line at the top of `visualize.py` / `visualize_sweep.py`; the figure still renders if the font is missing — labels just fall back to the default sans.

---
*This file was generated by Claude Code.*
