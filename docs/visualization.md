**English** | [日本語](visualization.ja.md)

# Visualization

The Python package `hegselmann-bc-tools` (a uv workspace member) reads the Rust outputs under `results/` and produces figures. Install once with `uv sync` at the workspace root.

```bash
uv sync
uv run hegselmann-bc-tools visualize
uv run hegselmann-bc-tools visualize-sweep
uv run hegselmann-bc-tools show-experiment-settings --results-dir results/latest
```

The unified CLI dispatches to one of three subcommands; arguments after the subcommand are passed straight to the corresponding module's argparse.

## `visualize` — opinion trajectory

Reads `opinions.csv` and `metrics.csv` from a `run` result (default `results/latest`) and writes two figures into `{results-dir}/figures/`:

- `opinion_trajectory.png` — the opinion trajectory (x = time, y = opinion in `[0,1]`, one translucent line per agent; paper Fig. 2 / 7 / 8 style). Final cluster centres are overlaid as dashed horizontal lines so plurality / polarization / consensus are visually obvious. The title shows the surviving-opinion count `n_surviving`.
- `metrics_timeseries.png` — three panels: `n_surviving` (log y), `variance`, and `max|Δx|` (log y, the convergence indicator).

```bash
uv run hegselmann-bc-tools visualize --results-dir results/latest
```

| Flag | Default | Description |
|---|---|---|
| `--results-dir` | results/latest | the run output directory |
| `--output-dir` | `{results-dir}/figures` | figure output directory |

## `visualize-sweep` — phase diagram

Reads `sweep_summary.csv` from a `sweep` result (default `results/latest`) and writes:

- `visualize_sweep.png` — a two-panel figure:
  - **Top:** mean number of surviving opinions vs ε with a ±1σ band (paper Fig. 3 / 12a style; the dashed line marks the consensus boundary at 1 cluster).
  - **Bottom:** mean final opinion vs ε with a ±1σ band (paper Fig. 12c style; for symmetric BC this should hug `x̄ = 0.5`, marked by the dashed reference line).

The numerical consensus brink `ε*` (smallest ε at which the trial-averaged surviving count first reaches 1) is also printed on stdout.

```bash
uv run hegselmann-bc-tools visualize-sweep --sweep-dir results/latest
```

| Flag | Default | Description |
|---|---|---|
| `--sweep-dir` | results/latest | the sweep output directory (`--results-dir` is also accepted) |
| `--output-dir` | `{sweep-dir}/figures` | figure output directory |

## `show-experiment-settings`

Pretty-prints the `config.json` (run) or `sweep_config.json` (sweep) found under a results directory; `results/latest` is resolved to its target. Use `--json` for machine-readable output.

```bash
uv run hegselmann-bc-tools show-experiment-settings --results-dir results/latest
uv run hegselmann-bc-tools show-experiment-settings --results-dir results/latest --json
```

## `reproduce` — batch paper-figure reproduction

Runs the Rust binary (`cargo run --release -- run / sweep ...`) once per Figure spec, reads back the produced CSVs, and writes a consolidated PNG per Figure into a single timestamped directory. Intermediate `opinions.csv` / `metrics.csv` / `sweep_summary.csv` outputs are kept under the cargo output root (`results/<inner_ts>(_sweep)?/`); their paths are recorded in `reproduce_summary.json`.

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
| `--cargo-output-dir` | same as `--output-dir` | passed as `--output-dir` to cargo for intermediate per-run CSVs |
| `--workspace-root` | (auto) | override the cargo workspace root (also via env `HEGSELMANN_BC_PROJECT_ROOT`) |
| `--quick` | off | shrink fig02 / fig03 / fig12 (n=125, runs=5) for a fast smoke run; do not use for paper-value verification |
| `--skip-build` | off | skip `cargo build --release` (assumes the binary is already built) |

## Note on fonts

The scripts request `font.family = "Hiragino Sans"` for Japanese labels (macOS). On other platforms, substitute an installed CJK font in the `plt.rcParams` line at the top of `visualize.py` / `visualize_sweep.py`; the figure still renders if the font is missing — labels just fall back to the default sans.

---
*This file was generated by Claude Code.*
