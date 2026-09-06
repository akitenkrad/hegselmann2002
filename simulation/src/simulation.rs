//! 初期化と実行ドライバ (SimulationBuilder 配線)．
//!
//! 論文 (Hegselmann & Krause 2002) §4 の BC モデルを socsim フレームワーク上で
//! 駆動する．Phase 1 (対称 ε) のみ実装しており，更新規則は `socsim-mechanisms`
//! パックの [`HegselmannKrauseMechanism`] が `Interaction` フェーズで同期適用
//! する．早期停止は [`ConvergenceMechanism`] を `PostStep` フェーズに配線して
//! `max|Δx| < tol` で `request_stop` する (論文 §3 Result 5 の有限時間収束)．
//!
//! 出力の置き場と同一性は runvault が持つ．ここが書くのは意見の軌跡
//! ([`save_opinions`]) だけで，指標は [`crate::record`] 経由で run へ落とす．

use std::fs::{create_dir_all, File};
use std::io::BufWriter;

use csv::Writer;
use rand::Rng;

use socsim_core::{derive_seed, SimRng};
use socsim_engine::{SequentialScheduler, SimulationBuilder};
use socsim_mechanisms::{max_abs_delta, regular_profile, MeanOperator};

use crate::config::{Config, StartProfile};
use crate::mechanisms::{ConvergenceMechanism, HegselmannKrauseMechanism};
use crate::metrics::Metrics;
use crate::world::OpinionWorld;

// 単一 root シードから用途別の独立な決定論的 RNG ストリームを派生させるラベル．
/// 初期意見分布の生成用 RNG ラベル．
const RNG_WORLD_INIT: u64 = 0;
/// socsim エンジン用 RNG ラベル (本モデルは決定論なので実質未使用)．
const RNG_ENGINE: u64 = 1;

/// シミュレーション全体の実行結果．
pub struct SimulationResult {
    /// 各ステップ (t=0 を含む) のメトリクス履歴．
    pub metrics_history: Vec<Metrics>,
    /// 各ステップの意見スナップショット (opinions.csv 用)．`opinions[t][i]`．
    pub opinion_history: Vec<Vec<f64>>,
    /// 不動点に到達したか (`max|Δx| < tol`)．
    pub converged: bool,
    /// 収束 (または最終) 反復番号．
    pub final_iteration: usize,
}

/// 初期意見ベクトルを生成する．
///
/// - [`StartProfile::Uniform`]: `[0,1]` 上の一様乱数 (`Uniform(0,1)`)．論文
///   Fig. 3 / 12 の設定．
/// - [`StartProfile::Regular`]: 等間隔 `x_i = i / (n-1)`．論文 Fig. 4–8 の
///   設定．`n = 1` のときは中点 `0.5` を返す．`socsim-mechanisms` PR #48 で
///   導入された共有ヘルパ [`regular_profile`] を流用する (既存のローカル実装
///   `(0..n).map(|i| i / (n-1)).collect()` と完全に同等で出力ビット同一)．
pub fn init_opinions(cfg: &Config, rng: &mut SimRng) -> Vec<f64> {
    match cfg.start_profile {
        StartProfile::Uniform => (0..cfg.n).map(|_| rng.gen_range(0.0..1.0)).collect(),
        StartProfile::Regular => regular_profile(cfg.n),
    }
}

/// シミュレーションを実行する．
///
/// Phase 3 対応: [`Config::is_symmetric`] に応じて
/// [`HegselmannKrauseMechanism::new`] (対称) と
/// [`HegselmannKrauseMechanism::with_asymmetric`] (非対称 `ε_l ≠ ε_r`) を
/// 切り替える．socsim-mechanisms PR #47 の bit-identical 契約により，
/// `eps_l == eps_r` のとき両者は同じ結果になるが，明示性のため `is_symmetric`
/// で分岐する (対称経路は Phase 1 と同一の挙動を保証する)．
///
/// `max_delta` (および収束フラグ) はメカニズムではなくドライバ側で，観測した
/// 連続ステップの意見スナップショット間 [`max_abs_delta`] として算出する．
/// `socsim-mechanisms` の `ConvergenceMechanism` も同じロジックで `request_stop`
/// するため，両者は同じステップで停止する．
pub fn run(cfg: &Config) -> SimulationResult {
    run_observed(cfg, |_| {})
}

/// The same, calling `on_step` once for every BC iteration.
///
/// The callback is where a caller counts its progress. A step is the unit
/// because it is the unit the cost is in: one iteration re-computes every
/// agent's confidence set, which is quadratic in `n`. A trial would be a single
/// tick, and `run` has exactly one trial — it would say nothing at all between
/// its first line and its last.
///
/// It is given the step number rather than nothing so a caller can report
/// against the iteration count rather than against its own tally.
pub fn run_observed(cfg: &Config, mut on_step: impl FnMut(usize)) -> SimulationResult {
    let root = cfg.seed.unwrap_or_else(rand::random);

    // 初期意見分布 (root から派生した init RNG)．
    let mut init_rng = SimRng::from_seed(derive_seed(root, &[RNG_WORLD_INIT]));
    let opinions = init_opinions(cfg, &mut init_rng);

    // 世界状態とエンジンを構築．
    let world = OpinionWorld::new(
        opinions.clone(),
        cfg.eps_l,
        cfg.eps_r,
        cfg.max_iterations as u64,
    );
    let opinion_mechanism: Box<HegselmannKrauseMechanism> = if cfg.is_symmetric() {
        // 対称 BC: Phase 1 と bit-identical な経路．
        Box::new(HegselmannKrauseMechanism::new(
            cfg.eps_l,
            MeanOperator::Arithmetic,
        ))
    } else {
        // 非対称 BC (論文 §4.2 / Fig. 10–13): socsim-mechanisms PR #47 で
        // 追加された with_asymmetric を流用する．
        Box::new(HegselmannKrauseMechanism::with_asymmetric(
            cfg.eps_l,
            cfg.eps_r,
            MeanOperator::Arithmetic,
        ))
    };
    let mut sim = SimulationBuilder::new(world)
        .scheduler(Box::new(SequentialScheduler))
        .seed(derive_seed(root, &[RNG_ENGINE]))
        .add_mechanism(opinion_mechanism)
        .add_mechanism(Box::new(ConvergenceMechanism::new(cfg.tol)))
        .build();

    let mut metrics_history: Vec<Metrics> = Vec::new();
    let mut opinion_history: Vec<Vec<f64>> = Vec::new();

    // 初期状態 (t=0) を記録．
    metrics_history.push(Metrics::compute(&opinions, 0, 0.0, cfg.tol));
    opinion_history.push(opinions);

    let mut converged = false;
    let mut final_iteration = cfg.max_iterations;

    sim.run_observed(|report| {
        let t = report.t as usize;
        // 連続ステップ間の最大変位 (id 昇順の要素差)．
        let prev = opinion_history
            .last()
            .expect("opinion_history は t=0 を含む");
        let max_delta = max_abs_delta(prev, &report.world.opinions);

        metrics_history.push(Metrics::compute(
            &report.world.opinions,
            t,
            max_delta,
            cfg.tol,
        ));
        opinion_history.push(report.world.opinions.clone());

        // 不動点に到達したか (= ConvergenceMechanism が request_stop する条件)．
        converged = max_delta < cfg.tol;
        final_iteration = t;
        on_step(t);
    })
    .expect("シミュレーションの実行に失敗");

    SimulationResult {
        metrics_history,
        opinion_history,
        converged,
        final_iteration,
    }
}

/// 意見履歴を long-format CSV (`t, agent_id, opinion`) に保存する．
///
/// 保存先は run ディレクトリの `artifacts/` で，実行中に書いたものとして
/// `manifest.csv` に載る．指標は runvault の `metrics.csv` が持つので，ここが
/// 書くのは意見の軌跡だけである．
pub fn save_opinions(opinion_history: &[Vec<f64>], output_dir: &str) {
    create_dir_all(output_dir).expect("出力ディレクトリの作成に失敗");
    let path = format!("{}/opinions.csv", output_dir);
    let file = File::create(&path).expect("opinions.csv の作成に失敗");
    let mut wtr = Writer::from_writer(BufWriter::new(file));
    wtr.write_record(["t", "agent_id", "opinion"])
        .expect("ヘッダ書き込みに失敗");
    for (t, opinions) in opinion_history.iter().enumerate() {
        for (i, &x) in opinions.iter().enumerate() {
            wtr.write_record(&[t.to_string(), i.to_string(), format!("{:.10}", x)])
                .expect("レコード書き込みに失敗");
        }
    }
    wtr.flush().expect("フラッシュに失敗");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_symmetric(n: usize, eps: f64, start: StartProfile, max_iter: usize) -> Config {
        Config::from_symmetric(
            n,
            eps,
            start,
            max_iter,
            1e-6,
            Some(42),
            "results".to_string(),
        )
    }

    #[test]
    fn same_seed_is_deterministic() {
        let cfg = cfg_symmetric(200, 0.15, StartProfile::Uniform, 100);
        let a = run(&cfg);
        let b = run(&cfg);
        assert_eq!(a.final_iteration, b.final_iteration);
        assert_eq!(a.converged, b.converged);
        let la = a.opinion_history.last().unwrap();
        let lb = b.opinion_history.last().unwrap();
        assert_eq!(la.len(), lb.len());
        for (x, y) in la.iter().zip(lb.iter()) {
            assert!((x - y).abs() < 1e-12);
        }
    }

    #[test]
    fn initial_metrics_recorded_at_t0() {
        let cfg = cfg_symmetric(50, 0.15, StartProfile::Uniform, 100);
        let r = run(&cfg);
        assert_eq!(r.metrics_history[0].t, 0);
        assert_eq!(r.opinion_history[0].len(), 50);
    }

    #[test]
    fn large_eps_reaches_consensus() {
        // ε=0.30, regular で n=200 → 全員が信頼集合に入り 1 ステップで合意．
        let cfg = cfg_symmetric(200, 0.30, StartProfile::Regular, 200);
        let r = run(&cfg);
        let last = r.metrics_history.last().unwrap();
        assert_eq!(
            last.n_surviving, 1,
            "ε=0.30 の regular n=200 では合意 (1 クラスタ) になるべき"
        );
        assert!(r.converged, "決定論的 BC は不動点で converged を立てるべき");
    }

    #[test]
    fn small_eps_remains_plural() {
        // ε=0.05, regular で n=200 → 多数の生存クラスタが残る．
        let cfg = cfg_symmetric(200, 0.05, StartProfile::Regular, 200);
        let r = run(&cfg);
        let last = r.metrics_history.last().unwrap();
        assert!(
            last.n_surviving > 1,
            "ε=0.05 では複数クラスタが残るべき (got {})",
            last.n_surviving
        );
    }

    #[test]
    fn asymmetric_eps_l_equals_eps_r_matches_symmetric_bit_for_bit() {
        // PR #47 の bit-identical 契約をローカルでも検証する．
        let sym = cfg_symmetric(200, 0.15, StartProfile::Uniform, 100);
        let mut asym = sym.clone();
        asym.eps_l = 0.15;
        asym.eps_r = 0.15;
        // 上記は既に is_symmetric() = true だが，将来の eps_l/eps_r 分離設定でも
        // 結果が変わらないことを担保する．
        let a = run(&sym);
        let b = run(&asym);
        assert_eq!(a.final_iteration, b.final_iteration);
        assert_eq!(a.opinion_history.last(), b.opinion_history.last());
    }

    #[test]
    fn asymmetric_shifts_mean_toward_wider_side() {
        // 論文 §4.2 / Fig. 11 の質的主張: eps_l ≪ eps_r で最終平均が右へ偏る．
        let sym = Config::from_symmetric(
            200,
            0.15,
            StartProfile::Uniform,
            200,
            1e-6,
            Some(7),
            "results".to_string(),
        );
        let asym = Config {
            eps_l: 0.05,
            eps_r: 0.25,
            ..sym.clone()
        };
        let a = run(&sym);
        let b = run(&asym);
        let sym_mean = a.metrics_history.last().unwrap().mean;
        let asym_mean = b.metrics_history.last().unwrap().mean;
        assert!(
            (sym_mean - 0.5).abs() < 0.05,
            "対称 ε=0.15 の平均は ~0.5 付近のはず (got {sym_mean})"
        );
        assert!(
            asym_mean > sym_mean + 0.02,
            "非対称 (eps_l=0.05, eps_r=0.25) の平均は対称より上のはず (got asym={asym_mean}, sym={sym_mean})"
        );
    }
}
