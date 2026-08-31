[English](cli.md) | **日本語**

# CLI

Rust バイナリ `hegselmann-bc` (`cargo run --release -- …` で実行) は `run` と `sweep` の 2 サブコマンドを提供する．

## `run` — 単一実行

対称 BC 力学を単一の `(n, ε)` で実行する．

```bash
cargo run --release -- run \
    --n 625 --eps 0.15 --start uniform \
    --max-iterations 100 --tol 1e-6 --seed 42
```

| フラグ | 既定値 | 説明 |
|---|---|---|
| `--n` | 625 | エージェント数 `n` |
| `--eps` | 0.15 | 対称信頼幅 `ε` (`--eps-l` / `--eps-r` 非指定時に両側に適用; `ε_l = ε_r = ε`) |
| `--eps-l` | なし | 非対称 BC モード時の左信頼幅 `ε_l` (論文 §4.2 / Fig. 10–13)．`--eps-r` と必ずペア指定．省略時は `--eps` の対称設定． |
| `--eps-r` | なし | 非対称 BC モード時の右信頼幅 `ε_r`．`--eps-l` と必ずペア指定． |
| `--start` | uniform | 初期意見プロファイル (`uniform` \| `regular`) |
| `--max-iterations` | 100 | 最大反復回数 `T` |
| `--tol` | 1e-6 | 収束許容誤差 (`max|Δx| < tol` で停止) |
| `--seed` | ランダム | RNG シード (省略時はランダム) |
| `--output-dir` | results | 出力ベースディレクトリ |

**出力ファイル:**

各実行は runvault の run ディレクトリとして保存される．run ディレクトリが出力先そのものなので，タイムスタンプ付きディレクトリも `latest` シンボリックリンクもこちらでは作らない．直近の完了 run のパスは `runvault` に聞く．

```bash
runvault path --experiment hegselmann-bc --latest --subcommand run --standalone
```

```
results/
└── hegselmann-bc/                                  ← experiment
    ├── latest_finished -> run_20260831_145450_...  ← 最後に完了した run
    ├── run_20260831_145450_c3b078ae_298d/          ← <subcommand>_<時刻>_<cfg8>_<exec4>
    │   ├── run.json                                ← メタデータ (git commit / 環境 / 論文情報)
    │   ├── config.json                             ← 封筒．実験条件は ["parameters"] の下
    │   ├── metrics.csv                             ← long 形式 (step / scope / name / value)
    │   ├── events.jsonl                            ← ステップごとの観測 + 終端行 (相のラベル)
    │   ├── status.json                             ← 終了状態と所要時間
    │   ├── manifest.csv                            ← artifacts/ と logs/ のハッシュ
    │   └── artifacts/
    │       └── opinions.csv                        ← long-format 意見軌跡: t, agent_id, opinion
    └── figures/                                    ← 可視化スクリプトの出力 (run の外)
        └── run_20260831_145450_c3b078ae_298d/
```

作図は run が終わった後に走るので，run ディレクトリの**外** (`<experiment>/figures/<run_slug>/`) に出す．`manifest.csv` は `finish()` が確定させるため，後から `artifacts/` に足したファイルにはハッシュが付かない．

`metrics.csv` は 1 行 1 値の long 形式である．ステップごとの 5 指標 `n_surviving` / `mean` / `variance` / `n_splits` / `max_delta` は `step` を持ち，run 全体を 1 つの値で表す `converged` (0.0 / 1.0) と `final_iteration` は `scope=run` で `step` を持たない．

**相 (`phase`) は指標ではない．** consensus / polarization / plurality はラベルであって数ではなく，しかも `n_surviving` から一意に決まる (≤1 / 2–10 / >10)．数を割り当てても情報は増えないので，最終的な相は `events.jsonl` の `terminal` 行に `"phase": "polarization"` のようにラベルのまま置く．同じ行が `outcome` / `censored` / `budget` で収束と打ち切りも表す．条件の表示は [`show-experiment-settings`](visualization.ja.md#show-experiment-settings) を参照．

### 相転移の例 (n = 625, uniform)

```bash
cargo run --release -- run --n 625 --eps 0.05 --seed 42   # → 多元 (多数クラスタ)
cargo run --release -- run --n 625 --eps 0.15 --seed 42   # → 分極 (2 陣営)
cargo run --release -- run --n 625 --eps 0.25 --seed 42   # → 合意 (1 陣営)
```

### 非対称 BC の例 (論文 §4.2 / Fig. 10–13)

```bash
# 非対称 BC (論文 Fig. 11 風: 右が広い → 最終平均が右へ偏る)
cargo run --release -- run --n 625 --eps-l 0.05 --eps-r 0.25 --start uniform --seed 42
```

## `sweep` — ε 走査

ε を範囲走査し，各試行の最終メトリクスを集計する．

```bash
cargo run --release -- sweep \
    --eps-min 0.01 --eps-max 0.40 --eps-step 0.01 \
    --n 625 --start uniform --runs 50 --seed 42
```

| フラグ | 既定値 | 説明 |
|---|---|---|
| `--eps-min` | 0.01 | ε 走査の最小値 |
| `--eps-max` | 0.40 | ε 走査の最大値 (含む) |
| `--eps-step` | 0.01 | ε 走査の刻み幅 |
| `--n` | 625 | エージェント数 |
| `--start` | uniform | 初期意見プロファイル (`uniform` \| `regular`) |
| `--runs` | 50 | 各 ε あたりの独立試行数 |
| `--max-iterations` | 100 | 最大反復回数 |
| `--tol` | 1e-6 | 収束許容誤差 |
| `--seed` | 42 | シード基点 (各試行は derive により独立化される) |
| `--output-dir` | results | 出力ベースディレクトリ |

各試行は `derive_seed(seed, &[eps.bits, run_id])` で独立なシードを派生させる．Sweep は単純化のため逐次実行 (rayon は使わない)．

**出力ファイル:**

sweep は「親 run 1 本 + ε ごとの子 run」として記録される．子は親の下ではなく experiment ディレクトリの兄弟として並び，`lineage.parent_run_uid` で親を指す．1 行 1 試行のサマリ CSV は書かない (同じ値は各子 run の `events.jsonl` にある)．

子のサブコマンド名は `run` ではなく `sweep-point` である．`run` は 1 本のシミュレーション，子は同一条件の `runs` 本で，中身の違う 2 つを同じ名前に同居させると `runvault path --subcommand run` がどちらを返すか分からなくなるためである．

```
results/
└── hegselmann-bc/
    ├── sweep_20260831_145451_23b44915_976d/         ← 親．parameters が ε グリッドの定義
    │   ├── run.json                                 ← lineage.sweep_id を持つ．rng.master_seed は null
    │   └── config.json
    ├── sweep-point_20260831_145451_2c10e235_bfc8/   ← 子 (ε 1 点)．lineage.parent_run_uid = 親の run_uid
    │   ├── config.json                              ← parameters に eps_l / eps_r / runs
    │   ├── metrics.csv                              ← 条件を 1 つの値で表す集約のみ (scope=run)
    │   └── events.jsonl                             ← 試行ごとに observation 1 行 + terminal 1 行
    └── ...
```

子の `events.jsonl` の `terminal` 行が，旧 `sweep_summary.csv` の 1 行に対応する — `unit_id` (`trial-<i>`) / `seed` / `t` (= `final_iteration`) / `censored` (= 収束の否定) / `n_surviving` / `mean` / `variance` / `n_splits` / `max_delta` / `phase`．試行ごとの値を `metrics.csv` に置くと (`run_uid`, `step`, `scope`, `name`) が重複するので，散らばりが要る図はこちらから組み直す．

親のパスは `runvault path --experiment hegselmann-bc --latest --subcommand sweep` で取れる．`hegselmann-bc-tools visualize-sweep` はこの親を受け取り，子 run を集めて従来のサマリ表を組み直す．

## Phase 3 ステータス

非対称 BC (`--eps-l` / `--eps-r`; 論文 §4.2 / Fig. 10–13) は本体側の `HegselmannKrauseMechanism::with_asymmetric` (socsim-mechanisms PR #47) を流用することで**対応済み**である．独自 mechanism は不要だった．Phase 3 の残作業は論文 Figure 一括再現 (`reproduce` サブコマンド) のみ．

---
*This file was generated by Claude Code.*
