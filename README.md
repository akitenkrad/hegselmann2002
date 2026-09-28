<p align="center"><img src="docs/assets/hero.svg" width="100%"></p>

**English** | [日本語](README.ja.md)

# Opinion Dynamics and Bounded Confidence — Hegselmann & Krause (2002)

A reimplementation of the bounded-confidence (BC) opinion-dynamics model of Hegselmann & Krause (2002), "Opinion Dynamics and Bounded Confidence: Models, Analysis and Simulation" (*JASSS* 5(3), 2). Each agent holds a continuous opinion `x_i ∈ [0,1]` and, every step, replaces it with the arithmetic mean of opinions inside its confidence set `I(i) = { j : |x_i − x_j| ≤ ε }`. As ε grows the outcome transitions from many surviving clusters (plurality) through two camps (polarization) to a single consensus. The simulation is written in Rust on top of the [socsim](https://github.com/akitenkrad/rs-social-simulation-tools) framework, and the visualization tools in Python.

## Install & Quick start

```bash
# Build the Rust simulation
cargo build --release

# Run with default settings (n=625, ε=0.15, uniform initial profile, seed=42)
cargo run --release -- run --n 625 --eps 0.15 --start uniform --seed 42

# Install the Python visualization tools (at the workspace root)
uv sync

# Visualize the most recent run (runvault path --latest picks it)
uv run hegselmann-bc-tools visualize
```


## Scratch runs

Use `--scratch` for development, debugging, and smoke-test runs. Scratch runs are created under `results/_scratch/`, are never synced to the vault, and the latest scratch run can be located with `runvault path --scratch`.

## Documentation

- [Use cases](docs/usecases.md) — what you can do with this project, with pointers to the rest of the docs.
- [CLI](docs/cli.md) — the Rust CLI: the `run` and `sweep` subcommands, their flags, and the runvault run layout under `results/`.
- [Visualization](docs/visualization.md) — the Python `hegselmann-bc-tools` and how to interpret the outputs.
- [Architecture](docs/architecture.md) — repository structure, the socsim framework, the BC update, and references.
- [Reproduction](docs/reproduction.md) — paper figure reproduction status (Phase 3, not yet started).

## Scope

This repository implements **Phase 1** (the symmetric BC model on the complete graph with the `run` subcommand and the three-phase transition; paper §4 baseline) and **Phase 2** (the `sweep` over ε plus the Python `visualize` / `visualize-sweep` / `show-experiment-settings` tools). **Phase 3** asymmetric confidence (`ε_l ≠ ε_r`, paper §4.2 / Fig. 10–13) is **also supported** via `--eps-l` / `--eps-r` on the `run` subcommand, driven by the upstream `HegselmannKrauseMechanism::with_asymmetric` (socsim-mechanisms PR #47) — no custom mechanism was needed. The remaining Phase 3 work is a one-shot `reproduce_paper.py` for batched paper-figure reproduction.

## Sister implementation

[`hegselmann2005`](https://github.com/akitenkrad/hegselmann2005) reimplements the 2005 *Computational Economics* paper by the same authors, which generalises the 2002 BC model along the axis of *which average* aggregates opinions (A / G / H / P_p / R). The two repositories share the socsim wiring pattern but stay deliberately separate (paper-keyed) — this repository owns the symmetric/asymmetric `ε` and the analytic finite-time-consensus results, that one owns the averaging-operator comparison.

## License

MIT
