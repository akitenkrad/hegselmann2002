[English](visualization.md) | **日本語**

# 可視化

Python パッケージ `hegselmann-bc-tools` (uv workspace メンバ) は runvault の run ディレクトリを読み，図を生成する．workspace ルートで `uv sync` を一度実行してインストールする．

```bash
uv sync
uv run hegselmann-bc-tools visualize
uv run hegselmann-bc-tools visualize-sweep
uv run hegselmann-bc-tools show-experiment-settings
```

統合 CLI は 3 つのサブコマンドへディスパッチする．サブコマンド以降の引数は対応モジュールの argparse へそのまま渡される．

**どの run を見るかは runvault が答える．** ディレクトリを指定しなければ `runvault path --experiment hegselmann-bc --latest --subcommand ...` が返す run を対象にするので，`results/` を走査して新しそうなディレクトリを当てにいくことはしない．`runvault` コマンドが PATH にあるか，環境変数 `RUNVAULT` がバイナリを指している必要がある．

```bash
cargo install --path <rs-runvault>/crates/runvault      # PATH に置く
export RUNVAULT=<rs-runvault>/target/debug/runvault     # または直接指す
```

**図は run ディレクトリの外に置く．** 出力先は `<results_root>/hegselmann-bc/figures/<run_slug>/` である．`manifest.csv` は `finish()` が確定させるので，run が終わった後に作るものを `artifacts/` に足すとハッシュが付かず，記録と食い違う．

runvault 以前の `results/<timestamp>/` も引き続き読める (`--results-dir` に直接渡す)．その場合は figures も従来どおり `<run>/figures/` に出る．

## `visualize` — 意見軌跡

`run` の run ディレクトリから `artifacts/opinions.csv` と `metrics.csv` (long 形式) を読み，2 枚の図を出力する:

- `opinion_trajectory.png` — 意見軌跡 (x = 時間，y = 意見 ∈ `[0,1]`，エージェントごとに 1 本の半透明線; 論文 Fig. 2 / 7 / 8 風)．最終クラスタ中心を破線水平線で重ね描き，多元・分極・合意が一目で判別できる．タイトルに生存意見数 `n_surviving` を表示する．
- `metrics_timeseries.png` — 3 段: `n_surviving` (log y) / `variance` / `max|Δx|` (log y，収束指標)．

```bash
uv run hegselmann-bc-tools visualize
uv run hegselmann-bc-tools visualize --results-dir "$(runvault path --experiment hegselmann-bc --latest --subcommand run --standalone)"
```

`--subcommand run --standalone` で絞っているので，sweep の親も子も掴むことはない．

| フラグ | 既定値 | 説明 |
|---|---|---|
| `--results-dir` | `runvault path --latest --subcommand run --standalone` | run ディレクトリ |
| `--results-root` | results | `--results-dir` 未指定時に runvault が探す results ルート |
| `--output-dir` | `<experiment>/figures/<run_slug>/` | 図の保存先ディレクトリ |

## `visualize-sweep` — 相図

sweep の親 run を受け取り，`lineage.parent_run_uid` で親を指す子 run (`sweep-point`) の `events.jsonl` を集めて 1 行 1 試行の表を組み直し，以下を出力する (`sweep_summary.csv` はもう書かれない)．±1σ バンドを描くには条件ごとの平均ではなく個々の試行が要るので，子の run スコープ集約ではなく終端行を読む:

- `visualize_sweep.png` — 上下 2 段の図:
  - **上段:** 生存意見数の平均 vs ε，±1σ バンド付き (論文 Fig. 3 / 12a 風; 合意境界 1 クラスタを破線で示す)．
  - **下段:** 最終平均意見 vs ε，±1σ バンド付き (論文 Fig. 12c 風; 対称 BC では `x̄ = 0.5` 近傍に張り付くはず．基準線を破線で示す)．

合意ブリンク数値 `ε*` (生存意見数の試行平均が初めて 1 に達する最小 ε) も標準出力に印字される．

```bash
uv run hegselmann-bc-tools visualize-sweep
uv run hegselmann-bc-tools visualize-sweep --sweep-dir "$(runvault path --experiment hegselmann-bc --latest --subcommand sweep)"
```

| フラグ | 既定値 | 説明 |
|---|---|---|
| `--sweep-dir` | `runvault path --latest --subcommand sweep` | sweep 親 run のディレクトリ (`--results-dir` も受け付ける) |
| `--results-root` | results | `--sweep-dir` 未指定時に runvault が探す results ルート |
| `--output-dir` | `<experiment>/figures/<run_slug>/` | 図の保存先ディレクトリ |

## `show-experiment-settings`

run ディレクトリの `config.json` から実験条件を整形表示する．`config.json` は runvault の封筒 (`schema_version` / `run_uid` / `runvault` / `parameters`) で，条件は `parameters` の下にある．`run` / `sweep` / `sweep-point` のどれかは `run.json` の `subcommand` が答えるので，どれを指定してもよい．legacy の flat な `config.json` / `sweep_config.json` も読める．機械可読出力は `--json` で得られる．

```bash
uv run hegselmann-bc-tools show-experiment-settings
uv run hegselmann-bc-tools show-experiment-settings --results-dir "$(runvault path --experiment hegselmann-bc --latest --subcommand sweep)"
uv run hegselmann-bc-tools show-experiment-settings --json
```

## `reproduce` — 論文 Figure 一括再現

Rust バイナリ (`cargo run --release -- run / sweep ...`) を Figure spec ごとに 1 回ずつ呼び出し，生成されたデータを読み込んで Figure ごとの PNG を 1 つのタイムスタンプ付きディレクトリにまとめる．中間データ (`artifacts/opinions.csv` / `metrics.csv` / `events.jsonl`) は runvault の run ディレクトリ (`results/hegselmann-bc/<run_slug>/`) に残り，そのパスは `reproduce_summary.json` に記録される．

どの run が今の呼び出しの出力かは `runvault path --latest` に聞くので，ディレクトリ名や mtime から推測しない．同一秒内に複数 spec が走っても衝突しないため，従来必要だった秒境界待ちのスリープも無くなった．

```bash
uv run hegselmann-bc-tools reproduce                  # フル再現 (論文値)
uv run hegselmann-bc-tools reproduce --quick          # 軽量モード (n=125, runs=5, 動作確認用)
uv run hegselmann-bc-tools reproduce --specs fig02,fig03
uv run hegselmann-bc-tools reproduce --skip-build     # 事前ビルド済みなら build をスキップ
```

対応 Figure 仕様 (論文 §4 のベンチマーク):

| Spec | サブコマンド | パラメータ | 期待される挙動 |
|---|---|---|---|
| `fig02` | `run` | `n=625, ε=0.01, uniform, max_iter=50, seed=42` | 約 38 クラスタ (fragmentation) |
| `fig07` | `run` | `n=100, ε=0.05, regular, max_iter=50, seed=1` | 8 splits (polarization) |
| `fig08` | `run` | `n=100, ε=0.25, regular, max_iter=30, seed=1` | 合意 (consensus) |
| `fig03` | `sweep` | `n=625, ε∈[0.01,0.40] step=0.01, runs=50, seed=42` | 生存意見数の急減 (3 相転移) |
| `fig12` | `fig03` の流用 | 同じ sweep データ | 最終平均意見 + 最終分散の 2 段プロット |
| `fig11` | `run` × 4 | `n=625, max_iter=100, seed=42`，非対称 `(ε_l,ε_r)∈{(.20,.20),(.15,.25),(.10,.30),(.05,.35)}` | 2×2 パネル．ε_r が広いほど最終平均が右へシフト |

出力構造:

```
results/reproduce_<YYYYMMDD_HHMMSS>/
├── reproduce_summary.json        # 各 spec の引数・cargo 呼び出し・状態・所要時間
└── figures/
    ├── fig02_n625_eps0.01_uniform.png
    ├── fig03_sweep_n_surviving.png
    ├── fig07_n100_eps0.05_regular.png
    ├── fig08_n100_eps0.25_regular.png
    ├── fig11_asymmetric_panel.png
    └── fig12_sweep_mean_variance.png
```

| フラグ | 既定値 | 説明 |
|---|---|---|
| `--specs` | (全て) | カンマ区切りで実行する spec ID (`fig02,fig03,fig07,fig08,fig11,fig12`) |
| `--output-dir` | results | 結果ルート (workspace 相対)．reproduce 一式は `<output-dir>/reproduce_<ts>/` に出る |
| `--cargo-output-dir` | `--output-dir` と同じ | cargo の `--output-dir` に渡す results ルート |
| `--workspace-root` | (自動) | cargo workspace ルートを上書き (環境変数 `HEGSELMANN_BC_PROJECT_ROOT` も可) |
| `--quick` | off | fig02 / fig03 / fig12 を縮小実行 (n=125, runs=5)．動作確認専用．論文値検証には使わない |
| `--skip-build` | off | `cargo build --release` をスキップ (ビルド済み前提) |

## フォントについて

スクリプトは `font.family = "Hiragino Sans"` を要求する (macOS の日本語ラベル用)．他プラットフォームでは `visualize.py` / `visualize_sweep.py` 冒頭の `plt.rcParams` で別の CJK フォントへ差し替えればよい．未インストールでも図自体は出力される (ラベルが既定の sans にフォールバックする)．
