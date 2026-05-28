//! Hegselmann & Krause (2002) 有界信頼意見力学の統合テスト．
//!
//! `hegselmann_bc_simulation` ライブラリクレートの公開 API に対して，
//! 論文 §4 の基本観察を smoke レベルで検証する:
//!
//! - 大きな ε → 合意 (生存意見数 1)
//! - ε=0 → 意見が不変
//! - 同一 seed の決定論性 (最終意見が完全一致)
//! - regular + 中程度 ε → 多元 → 分極の方向に向かう (n_surviving > 1)

use hegselmann_bc_simulation::config::{Config, StartProfile};
use hegselmann_bc_simulation::simulation::run;

fn base_config(n: usize, eps: f64, start: StartProfile, max_iter: usize) -> Config {
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
fn large_eps_reaches_consensus() {
    // ε=0.5 なら uniform でもどの意見も互いに信頼 → 1 ステップで合意．
    let cfg = base_config(100, 0.5, StartProfile::Uniform, 200);
    let r = run(&cfg);
    let last = r.metrics_history.last().unwrap();
    assert_eq!(last.n_surviving, 1, "ε=0.5 では合意になるべき");
    assert!(r.converged, "決定論的 BC は不動点で converged を立てるべき");
}

#[test]
fn zero_eps_leaves_opinions_unchanged() {
    // ε=0 なら信頼集合は自分自身のみ → 算術平均は自分の意見そのもの → 不変．
    let cfg = base_config(50, 0.0, StartProfile::Uniform, 50);
    let r = run(&cfg);
    let initial = &r.opinion_history[0];
    let last = r.opinion_history.last().unwrap();
    assert_eq!(initial.len(), last.len());
    for (a, b) in initial.iter().zip(last.iter()) {
        assert!((a - b).abs() < 1e-12, "ε=0 では意見は変化しないはず");
    }
    assert!(r.converged);
}

#[test]
fn same_seed_is_fully_reproducible() {
    let a = run(&base_config(150, 0.15, StartProfile::Uniform, 100));
    let b = run(&base_config(150, 0.15, StartProfile::Uniform, 100));
    assert_eq!(a.final_iteration, b.final_iteration);
    assert_eq!(a.converged, b.converged);
    let la = a.opinion_history.last().unwrap();
    let lb = b.opinion_history.last().unwrap();
    for (x, y) in la.iter().zip(lb.iter()) {
        assert!((x - y).abs() < 1e-12, "同一 seed の BC は完全再現");
    }
}

#[test]
fn small_eps_regular_remains_plural() {
    // ε=0.05, regular n=100 → 論文 Fig. 7 系列で複数 split が残る．
    let cfg = base_config(100, 0.05, StartProfile::Regular, 100);
    let r = run(&cfg);
    let last = r.metrics_history.last().unwrap();
    assert!(
        last.n_surviving > 1,
        "ε=0.05 regular では複数クラスタが残るべき (got {})",
        last.n_surviving,
    );
}
