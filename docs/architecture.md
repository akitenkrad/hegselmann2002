**English** | [日本語](architecture.ja.md)

# Architecture

## Repository structure

A two-project layout: a Cargo workspace + a uv workspace.

```
hegselmann2002/
├── Cargo.toml                 # Cargo workspace root (members = ["simulation"])
├── pyproject.toml             # uv workspace root (members = ["tools"])
├── simulation/                # Rust project (hegselmann-bc-simulation)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs            # CLI (run / sweep)
│   │   ├── lib.rs             # module re-exports for the binary + integration tests
│   │   ├── config.rs          # Config + config.json serialization
│   │   ├── world.rs           # socsim WorldState impl (OpinionWorld, complete graph)
│   │   ├── mechanisms.rs      # socsim Mechanism re-exports (HegselmannKrauseMechanism / ConvergenceMechanism)
│   │   ├── metrics.rs         # surviving opinions, splits, phase classification
│   │   └── simulation.rs      # init + run driver (SimulationBuilder wiring)
│   └── tests/
│       └── integration_test.rs
├── tools/                     # Python project (hegselmann-bc-tools)
│   ├── pyproject.toml
│   └── src/hegselmann_bc_tools/
│       ├── cli.py                       # unified CLI (hegselmann-bc-tools)
│       ├── visualize.py                 # opinion trajectory + metrics
│       ├── visualize_sweep.py           # phase diagram (n_surviving × ε + mean × ε)
│       └── show_experiment_settings.py  # display run / sweep settings
├── docs/                      # bilingual documentation
└── results/                   # simulation output (gitignored)
```

- `cargo run` launches the `simulation` crate from the workspace root.
- `uv run` invokes the `hegselmann-bc-tools` command exposed by the `tools` member of the uv workspace.

## Model on the socsim framework

The simulation engine is built on the social-simulation framework [rs-social-simulation-tools](https://github.com/akitenkrad/rs-social-simulation-tools) (socsim) — a git dependency, with the commit pinned in `Cargo.lock`. Because the canonical Hegselmann–Krause model is a **complete-graph / non-spatial** model, only `socsim-core` (traits), `socsim-engine` (Simulation / Builder), `socsim-mechanisms` (the HK / convergence pack), `socsim-metrics` (statistics) and `socsim-results` (output helpers) are used — there is **no `socsim-grid` and no `socsim-net`**.

The socsim APIs used:

- `WorldState` — `OpinionWorld` implements `agent_ids` / `clock` / `clock_mut`. It holds the opinion vector `opinions: Vec<f64>` and the confidence radii `eps_l` / `eps_r` (Phase 1 uses `eps_l == eps_r`).
- `ScalarOpinions` + `Neighbors` — the capability traits required by the pack's `HegselmannKrauseMechanism<W>`. `Neighbors::neighbors_of(i)` returns every agent except `i` in id order (complete graph); the ε-confidence filter is applied inside the mechanism.
- `HegselmannKrauseMechanism::new(ε, MeanOperator::Arithmetic)` (Interaction phase) — the synchronous BC update.
- `ConvergenceMechanism::new(tol)` (PostStep phase) — `request_stop` once `max|Δx| < tol`.
- `SequentialScheduler` — agents are activated in ascending `AgentId` order. Synchronous updates make the order irrelevant, but the deterministic scheduler keeps runs reproducible.
- `Simulation::run_observed` — the driver records per-step opinion snapshots and metrics via the observer callback.
- `SimRng` / `derive_seed` — `derive_seed(root, &[0])` seeds the initial-opinion RNG (label `RNG_WORLD_INIT`); `derive_seed(root, &[1])` seeds the engine RNG (label `RNG_ENGINE`, unused by BC since the update is deterministic).

## The bounded-confidence update (synchronous)

The update rule (paper §4) is

```
x_i(t+1) = (1 / |I(i, x(t))|) Σ_{j ∈ I(i, x(t))} x_j(t)
           I(i, x) = { j : |x_i − x_j| ≤ ε } ∪ {i}
```

The mechanism (in `socsim-mechanisms`) implements **synchronous (simultaneous) update**: it snapshots `prev = opinions` at the start of the step, computes each agent's confidence set and new opinion from `prev`, then writes all new opinions at once. The update is deterministic and order-independent. Complete-graph scan is `O(n²)` per step; the paper's `n = 625` is lightweight.

## Initial profiles

Two initial-opinion profiles are supported (paper §4):

- `uniform` — `x_i ~ Uniform(0, 1)` (Fig. 3 / 12).
- `regular` — equidistant `x_i = i / (n − 1)` (Fig. 4–8). For `n = 1` the midpoint `0.5` is returned.

Both are deterministic given the seed; only `uniform` consumes RNG draws.

## Metrics

`metrics.csv` records per step: `t, n_surviving, mean, variance, n_splits, phase, max_delta`.

- `n_surviving` — number of surviving distinct opinion clusters, computed by `socsim_metrics::stats::distinct_clusters(opinions, CLUSTER_TOL)` with `CLUSTER_TOL = 1e-4` (greedy single-linkage on the sorted opinions; gap > tol starts a new cluster). Paper §4 *surviving opinions* / Fig. 12 *number of opinions at convergence*.
- `n_splits` — `n_surviving − 1` (paper Fig. 7 "8 splits"). Equivalent definition: how many neighboring gaps in the sorted opinion profile exceed `CLUSTER_TOL`.
- `mean`, `variance` — from `socsim_metrics::stats::{mean, variance}`.
- `max_delta` — from `socsim_mechanisms::max_abs_delta(prev, curr)`, the convergence diagnostic.
- `phase` — `consensus = 1` (n ≤ 1), `polarization = 2` (2..=10), `plurality = 3` (> 10). The numeric thresholds are this repository's choice for readability; the paper itself classifies qualitatively.

Paper-specific metrics (`n_splits`, `phase`) are kept as **local implementations** because their meaning is anchored in this paper; the canonical statistical primitives stay shared via `socsim-metrics`.

## Reproducibility & determinism

For a given seed the run is fully deterministic: BC is a deterministic dynamical system, the engine RNG is only seeded for completeness, and the only stochastic element is the `uniform` initial profile, which derives its RNG from `derive_seed(seed, &[RNG_WORLD_INIT])`. Two runs with the same seed yield identical opinion trajectories down to floating-point bits.

## Future extensions (Phase 3, not started)

The design keeps clean extension points for:

- **Asymmetric confidence `ε_l ≠ ε_r`** — paper §4 (Fig. 10–13) defines `I(i) = { j : −ε_l ≤ x_j − x_i ≤ ε_r }`. The current `socsim-mechanisms::HegselmannKrauseMechanism` only supports a single symmetric `epsilon`, so Phase 3 will add a local `AsymmetricHegselmannKrauseMechanism { eps_l, eps_r }`, then upstream it to socsim. `OpinionWorld` already carries both `eps_l` and `eps_r`; `Config::is_symmetric` exposes the dispatch point; and `simulation::run` currently `assert!`s on the symmetric path.
- **Paper figure reproduction** — a `reproduce_paper.py` that batch-runs the Rust binary and reproduces Fig. 2 / 3 / 7 / 8 / 12 etc.
- **Analytic finite-time consensus checks** — small-`n` cases (`n ≤ 4` consensus ⟺ ε-profile; `n = 5, 6` counterexamples) verified numerically.

## References

- Hegselmann, R., & Krause, U. (2002). Opinion Dynamics and Bounded Confidence: Models, Analysis and Simulation. *JASSS*, 5(3), 2.
- Hegselmann, R., & Krause, U. (2005). Opinion Dynamics Driven by Various Ways of Averaging. *Computational Economics*, 25, 381–405. (Sister reimplementation: [`hegselmann2005`](https://github.com/akitenkrad/hegselmann2005).)
- Deffuant, G., Neau, D., Amblard, F., & Weisbuch, G. (2000). Mixing beliefs among interacting agents. *Advances in Complex Systems*, 3, 87–98.

---
*This file was generated by Claude Code.*
