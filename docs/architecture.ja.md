[English](architecture.md) | **日本語**

# アーキテクチャ

## リポジトリ構成

Cargo workspace + uv workspace の 2 プロジェクト構成．

```
hegselmann2002/
├── Cargo.toml                 # Cargo workspace ルート (members = ["simulation"])
├── pyproject.toml             # uv workspace ルート (members = ["tools"])
├── simulation/                # Rust プロジェクト (hegselmann-bc-simulation)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs            # CLI (run / sweep)
│   │   ├── lib.rs             # バイナリ + 統合テスト用のモジュール再エクスポート
│   │   ├── config.rs          # Config + runvault の parameters シリアライズ
│   │   ├── world.rs           # socsim WorldState 実装 (OpinionWorld，完全グラフ)
│   │   ├── mechanisms.rs      # socsim Mechanism 再エクスポート (HegselmannKrauseMechanism / ConvergenceMechanism)
│   │   ├── metrics.rs         # 生存意見数・分裂数・相分類
│   │   ├── record.rs          # runvault への記録 (論文メタデータ / 指標 / 終端イベント)
│   │   └── simulation.rs      # 初期化 + run ドライバ (SimulationBuilder 配線)
│   └── tests/
│       └── integration_test.rs
├── tools/                     # Python プロジェクト (hegselmann-bc-tools)
│   ├── pyproject.toml
│   └── src/hegselmann_bc_tools/
│       ├── cli.py                       # 統合 CLI (hegselmann-bc-tools)
│       ├── visualize.py                 # 意見軌跡 + メトリクス
│       ├── visualize_sweep.py           # 相図 (n_surviving × ε + mean × ε)
│       └── show_experiment_settings.py  # run / sweep 設定の表示
├── docs/                      # バイリンガル ドキュメント
└── results/                   # シミュレーション出力 (gitignore)
```

- `cargo run` は workspace ルートから `simulation` クレートを起動する．
- `uv run` は uv workspace の `tools` メンバが公開する `hegselmann-bc-tools` コマンドを起動する．

## socsim フレームワーク上のモデル

シミュレーション基盤は社会シミュレーション基盤 [rs-social-simulation-tools](https://github.com/akitenkrad/rs-social-simulation-tools) (socsim) ── git 依存とし，commit は `Cargo.lock` で固定する．正準 Hegselmann–Krause モデルは **完全グラフ・非空間** モデルなので，`socsim-core` (traits)・`socsim-engine` (Simulation / Builder)・`socsim-mechanisms` (HK / Convergence パック)・`socsim-metrics` (統計) のみを使う ── **`socsim-grid` も `socsim-net` も使わない**．出力の置き場と同一性は [runvault](https://github.com/akitenkrad/rs-runvault) が持つので，`socsim-results` (timestamp / latest シンボリックリンク / CSV・JSON 書き出し) も使わない．

使用する socsim API:

- `WorldState` — `OpinionWorld` が `agent_ids` / `clock` / `clock_mut` を実装．意見ベクトル `opinions: Vec<f64>` と左右の信頼幅 `eps_l` / `eps_r` (Phase 1 では `eps_l == eps_r`) を保持する．
- `ScalarOpinions` + `Neighbors` — パックの `HegselmannKrauseMechanism<W>` が要求する能力トレイト．`Neighbors::neighbors_of(i)` は自分以外の全エージェントを id 昇順で返す (完全グラフ)．ε による信頼集合の絞り込みはメカニズム内で行われる．
- `HegselmannKrauseMechanism::new(ε, MeanOperator::Arithmetic)` (Interaction フェーズ) — 同期 BC 更新．
- `ConvergenceMechanism::new(tol)` (PostStep フェーズ) — `max|Δx| < tol` で `request_stop`．
- `SequentialScheduler` — エージェントは id 昇順に活性化される．同期更新では順序は無関係だが，決定論的スケジューラを使うことで再現性を保つ．
- `Simulation::run_observed` — ドライバはオブザーバコールバック経由で各ステップの意見スナップショットとメトリクスを記録する．
- `SimRng` / `derive_seed` — `derive_seed(root, &[0])` で初期意見 RNG (ラベル `RNG_WORLD_INIT`)，`derive_seed(root, &[1])` でエンジン RNG (ラベル `RNG_ENGINE`，BC は決定論なので未使用) を派生させる．

## 有界信頼更新 (同期)

更新規則 (論文 §4):

```
x_i(t+1) = (1 / |I(i, x(t))|) Σ_{j ∈ I(i, x(t))} x_j(t)
           I(i, x) = { j : |x_i − x_j| ≤ ε } ∪ {i}
```

メカニズム (in `socsim-mechanisms`) は **同期 (simultaneous) 更新** を実装する: ステップ開始時に `prev = opinions` のスナップショットを取り，各エージェントの信頼集合と新意見を `prev` から計算してから，新意見を一括代入する．更新は決定論的かつ順序非依存．完全グラフ走査は 1 ステップ `O(n²)`；論文の `n = 625` でも軽量．

## 初期分布

論文 §4 に従い 2 種類の初期分布をサポートする:

- `uniform` — `x_i ~ Uniform(0, 1)` (Fig. 3 / 12)．
- `regular` — 等間隔 `x_i = i / (n − 1)` (Fig. 4–8)．`n = 1` の場合は中点 `0.5`．

両者ともシードに対して決定論的．RNG を消費するのは `uniform` のみ．

## メトリクス

`metrics.csv` は runvault の long 形式 (`run_uid, step, step_unit, scope, name, value`) で，ステップごとに `n_surviving` / `mean` / `variance` / `n_splits` / `max_delta` を `step_unit=step`・`scope=run` で記録する．run 全体を表す `converged` / `final_iteration` は `step` を持たない行として同じファイルに入る．

- `n_surviving` — 生存意見クラスタ数．`socsim_metrics::stats::distinct_clusters(opinions, CLUSTER_TOL)` (`CLUSTER_TOL = 1e-4`) で計算する (ソート列の隣接ペアのギャップ > tol を新クラスタの開始とみなす greedy single-linkage)．論文 §4 *surviving opinions* / Fig. 12 *number of opinions at convergence* に対応する．
- `n_splits` — `n_surviving − 1` (論文 Fig. 7 "8 splits")．等価な定義: ソート済み意見プロファイル中で隣接ペアのギャップが `CLUSTER_TOL` を超える箇所の数．
- `mean`, `variance` — `socsim_metrics::stats::{mean, variance}` を流用．
- `max_delta` — `socsim_mechanisms::max_abs_delta(prev, curr)`，収束診断．
- `phase` — `consensus` (n ≤ 1)，`polarization` (2..=10)，`plurality` (> 10)．区分の閾値は本リポジトリの可読性のための選択で，論文自身は定性的に分類している．

`phase` は **指標ではない**．ラベルであって数ではないので long 形式の `value` 列には載らず，しかも同じ行の `n_surviving` から一意に決まるので，数を割り当てても情報は増えない．最終的な相は `events.jsonl` の `terminal` 行にラベルのまま置く (`"phase": "polarization"`)．

論文固有の量 (`n_splits` / `phase`) は本論文での意味付けに依拠するため **ローカル実装** として残す．正準的な統計プリミティブだけが `socsim-metrics` に共有されている．

## 再現性・決定論性

同一シードに対して実行は完全決定論的: BC は決定論的力学系であり，エンジン RNG は形式上の seed 配線のみで実際には使われず，唯一の確率要素である `uniform` 初期分布も `derive_seed(seed, &[RNG_WORLD_INIT])` から派生する．同一シードの 2 実行は意見軌跡が浮動小数点ビットレベルで一致する．

## 将来拡張 (Phase 3，未着手)

以下の拡張点を残してある:

- **非対称信頼 `ε_l ≠ ε_r`** — 論文 §4 (Fig. 10–13) は `I(i) = { j : −ε_l ≤ x_j − x_i ≤ ε_r }` を定義する．現行 `socsim-mechanisms::HegselmannKrauseMechanism` は対称 `epsilon` 単一フィールドしか持たないため，Phase 3 ではローカルで `AsymmetricHegselmannKrauseMechanism { eps_l, eps_r }` を追加し，socsim 本体へ upstream する．`OpinionWorld` は両方の `eps_l` / `eps_r` を保持済み，`Config::is_symmetric` がディスパッチ点を提供し，`simulation::run` は現状対称経路で `assert!` する．
- **論文 Figure の一括再現** — Rust バイナリをバッチで呼ぶ `reproduce_paper.py` を追加し，Fig. 2 / 3 / 7 / 8 / 12 等を再現する．
- **解析的有限時間合意の数値検証** — 小さな `n` の場合分け (`n ≤ 4` で合意 ⟺ ε-profile; `n = 5, 6` の反例) を数値的に検証する．

## 参考文献

- Hegselmann, R., & Krause, U. (2002). Opinion Dynamics and Bounded Confidence: Models, Analysis and Simulation. *JASSS*, 5(3), 2.
- Hegselmann, R., & Krause, U. (2005). Opinion Dynamics Driven by Various Ways of Averaging. *Computational Economics*, 25, 381–405. (姉妹再現実装: [`hegselmann2005`](https://github.com/akitenkrad/hegselmann2005))
- Deffuant, G., Neau, D., Amblard, F., & Weisbuch, G. (2000). Mixing beliefs among interacting agents. *Advances in Complex Systems*, 3, 87–98.

---
*This file was generated by Claude Code.*
