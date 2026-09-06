**English** | [日本語](usecases.ja.md)

# Use cases

This project reproduces the symmetric bounded-confidence (BC) opinion-dynamics model of Hegselmann & Krause (2002).

## What you can do

1. **Observe the three-phase transition.** Run a single simulation and watch how the confidence radius ε controls the outcome: small ε leaves many opinion clusters (plurality), intermediate ε produces two camps (polarization), and large ε yields consensus. See [CLI → run](cli.md) and [Visualization](visualization.md).

   ```bash
   cargo run --release -- run --n 625 --eps 0.05 --start uniform --seed 42   # plurality
   cargo run --release -- run --n 625 --eps 0.15 --start uniform --seed 42   # polarization
   cargo run --release -- run --n 625 --eps 0.25 --start uniform --seed 42   # consensus
   uv run hegselmann-bc-tools visualize
   ```

2. **Map the phase diagram with an ε sweep.** Run `sweep` to scan ε across `[0.01, 0.40]` and aggregate the mean number of surviving opinions per ε. Paper §4 reports that `ε > 0.4` always reaches consensus regardless of `n` or initial profile; the sweep diagram makes this concrete.

   ```bash
   cargo run --release -- sweep --eps-min 0.01 --eps-max 0.40 --eps-step 0.01 \
       --n 625 --start uniform --runs 50 --seed 42
   uv run hegselmann-bc-tools visualize-sweep
   ```

3. **Compare initial profiles.** Switch `--start uniform` ↔ `--start regular` and observe how the deterministic equidistant profile (`x_i = i/(n-1)`) produces clean integer split counts (paper Fig. 4–8), while the random uniform profile produces noisy but consistent statistics (Fig. 3 / 12).

   ```bash
   cargo run --release -- run --n 100 --eps 0.05 --start regular --seed 1   # Fig. 7 setting (≈ 8 splits)
   uv run hegselmann-bc-tools visualize
   ```

## Where to go next

- [CLI](cli.md) — the full flag reference for `run` and `sweep`.
- [Visualization](visualization.md) — the Python tools and how to read the figures.
- [Architecture](architecture.md) — the model, the socsim wiring, and the metric definitions.
- [Reproduction](reproduction.md) — paper figure reproduction status.
