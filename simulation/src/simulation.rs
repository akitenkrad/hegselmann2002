//! 初期化と実行ドライバ (SimulationBuilder 配線)．
//!
//! 論文 (Hegselmann & Krause 2002) §4 の BC モデルを socsim フレームワーク上で
//! 駆動する．Phase 1 (対称 ε) のみ実装しており，更新規則は `socsim-mechanisms`
//! パックの [`HegselmannKrauseMechanism`] が `Interaction` フェーズで同期適用
//! する．早期停止は [`ConvergenceMechanism`] を `PostStep` フェーズに配線して
//! `max|Δx| < tol` で `request_stop` する (論文 §3 Result 5 の有限時間収束)．

use std::fs::File;
use std::io::BufWriter;

use csv::Writer;
use rand::Rng;

use socsim_core::{derive_seed, SimRng};
use socsim_engine::{SequentialScheduler, SimulationBuilder};
use socsim_mechanisms::{max_abs_delta, MeanOperator};

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
///   設定．`n = 1` のときは中点 `0.5` を返す．
pub fn init_opinions(cfg: &Config, rng: &mut SimRng) -> Vec<f64> {
    match cfg.start_profile {
        StartProfile::Uniform => (0..cfg.n).map(|_| rng.gen_range(0.0..1.0)).collect(),
        StartProfile::Regular => {
            if cfg.n <= 1 {
                vec![0.5; cfg.n]
            } else {
                let denom = (cfg.n - 1) as f64;
                (0..cfg.n).map(|i| i as f64 / denom).collect()
            }
        }
    }
}

/// シミュレーションを実行する．
///
/// Phase 1 では `cfg.eps_l == cfg.eps_r` を仮定し，対称 BC として
/// [`HegselmannKrauseMechanism::new`] を `cfg.eps_l` で構築する．非対称
/// (`eps_l != eps_r`) を渡すと panic する — Phase 3 の独自 mechanism で対応する
/// 設計余地として残してある (TODO はメカニズムモジュール参照)．
///
/// `max_delta` (および収束フラグ) はメカニズムではなくドライバ側で，観測した
/// 連続ステップの意見スナップショット間 [`max_abs_delta`] として算出する．
/// `socsim-mechanisms` の `ConvergenceMechanism` も同じロジックで `request_stop`
/// するため，両者は同じステップで停止する．
pub fn run(cfg: &Config) -> SimulationResult {
    assert!(
        cfg.is_symmetric(),
        "Phase 1 では対称 BC (eps_l == eps_r) のみサポートする (eps_l={}, eps_r={}). \
         非対称 BC は Phase 3 で AsymmetricHegselmannKrauseMechanism に切り替え予定．",
        cfg.eps_l,
        cfg.eps_r,
    );
    let eps = cfg.eps_l;

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
    let mut sim = SimulationBuilder::new(world)
        .scheduler(Box::new(SequentialScheduler))
        .seed(derive_seed(root, &[RNG_ENGINE]))
        .add_mechanism(Box::new(HegselmannKrauseMechanism::new(
            eps,
            MeanOperator::Arithmetic,
        )))
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
pub fn save_opinions(opinion_history: &[Vec<f64>], output_dir: &str) {
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

/// メトリクス履歴を CSV に保存する．
///
/// 書き出し機構は `socsim_results::write_csv` に委譲する (各行を `serialize`
/// し先頭行にヘッダを書く csv クレートの標準挙動)．
pub fn save_metrics(metrics: &[Metrics], output_dir: &str) {
    let path = format!("{}/metrics.csv", output_dir);
    socsim_results::write_csv(metrics, &path).expect("metrics.csv の書き込みに失敗");
}

/// 出力ディレクトリを作成する (idempotent)．
pub fn ensure_output_dir(output_dir: &str) {
    socsim_results::ensure_dir(output_dir).expect("出力ディレクトリの作成に失敗");
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
    #[should_panic(expected = "Phase 1 では対称 BC")]
    fn asymmetric_eps_panics_in_phase_1() {
        let mut cfg = cfg_symmetric(20, 0.15, StartProfile::Uniform, 10);
        cfg.eps_r = 0.30; // 非対称化
        let _ = run(&cfg);
    }
}
