//! runvault への記録の共通部分．
//!
//! 論文メタデータ (research) は `run` / `sweep` のどちらでも同一なので，ここ
//! 1 箇所で組み立てる．ステップごとの指標，シミュレーション 1 本の終端行，
//! 条件 1 点ぶんの集約もここに集める．

use runvault::{Replication, Run, Target, Work};
use serde::Serialize;

use crate::metrics::{Metrics, Phase};
use crate::simulation::SimulationResult;

/// runvault 上の実験名．`runvault path --experiment` に渡す値でもある．
/// バイナリ名と揃えて，姉妹実装 `hegselmann2005` と取り違えないようにする．
pub const EXPERIMENT: &str = "hegselmann-bc";
/// リポジトリの安定 id．git remote の名前とは独立に固定する．
pub const REPO_ID: &str = "hegselmann2002";
/// 分野．初期意見の一様乱数を引くので `simulation` (= `master_seed` が必須)．
pub const DOMAIN: &str = "simulation";

/// 時間軸の単位．
///
/// BC モデルの時間は全エージェントを同期更新する離散ラウンドで，論文 §4 の
/// $t = 0, 1, 2, \dots$ そのものである．runvault の語彙では `step`．
const T_UNIT: &str = "step";

/// この再現実験が対象としている論文．
///
/// JASSS 5(3), 2 には DOI が無い (設計書の書誌表も「N/A」) ので，同定は vault の
/// `paper-id` で行う．どの Figure を掴むかは `--n` / `--eps` / `--start` 次第で
/// run ごとに変わるため，`Target::figure` はここでは付けない (ε の相転移という
/// claim だけを共通の対象として持つ)．
pub fn replication() -> Replication {
    Work::paper_id("P00001404")
        .title("Opinion Dynamics and Bounded Confidence: Models, Analysis and Simulation")
        .year(2002)
        .source_version("published")
        .target(Target::claim(
            "epsilon-driven-phase-transition",
            "Widening the confidence level ε turns plurality into polarization and then into consensus",
        ))
        .obsidian_note("研究/98_論文レポート/80-再現実験/実装完了/hegselmann2002/設計書.md")
}

// ---------------------------------------------------------------------------
// ステップごとの指標
// ---------------------------------------------------------------------------

/// シミュレーション 1 本ぶんの記録 (`run` サブコマンド用)．
///
/// ステップごとの 5 指標 (`t` は時間軸なので値としては書かない) と，run 全体を
/// 1 つの値で表す `converged` / `final_iteration` を書く．
pub fn log_simulation(run: &mut Run, result: &SimulationResult) {
    for m in &result.metrics_history {
        log_step(run, m);
    }
    run.log_metrics(
        "run",
        &[
            ("converged", if result.converged { 1.0 } else { 0.0 }),
            ("final_iteration", result.final_iteration as f64),
        ],
    )
    .expect("run スコープの指標の記録に失敗");
}

/// `Metrics` の数値フィールドを 1 ステップぶんまとめて書く．
///
/// `Metrics::phase` はここに来ない．相 (consensus / polarization / plurality) は
/// 数ではなく category なので指標にはできず，しかも `Phase::classify(n_surviving)`
/// で同じ行の `n_surviving` から一意に決まる — 数字を割り当てても情報は増えない．
/// 最終的な相は [`log_terminal`] が `events.jsonl` にラベル文字列で書く．
fn log_step(run: &mut Run, m: &Metrics) {
    run.log_metrics_at(
        m.t as u64,
        T_UNIT,
        "run",
        &[
            ("n_surviving", m.n_surviving as f64),
            ("mean", m.mean),
            ("variance", m.variance),
            ("n_splits", m.n_splits as f64),
            ("max_delta", m.max_delta),
        ],
    )
    .unwrap_or_else(|e| panic!("step {} の指標の記録に失敗: {e}", m.t));
}

// ---------------------------------------------------------------------------
// 終端イベント
// ---------------------------------------------------------------------------

/// `events.jsonl` に書く観測行．
///
/// 予約キーだけを持つ．数はここには書かない — ステップごとの値は `metrics.csv`
/// (run スコープ) が，試行の最終値は下の [`TerminalEvent`] が正本なので，同じ数を
/// 2 箇所に置くと食い違う余地ができる．この行が持つのは「その単位をいつ見たか」
/// という時間軸だけである．
///
/// `terminal` 行だけでも生存時間解析は組めるが (`schema/v1/event.json` の
/// terminal の注記)，`runvault verify --deep` は terminal の `unit_id` が
/// observation にも現れることを要求するので，観測した時刻を明示的に残す．
#[derive(Serialize)]
struct ObservationEvent<'a> {
    unit_id: &'a str,
    t: u64,
    t_unit: &'static str,
}

/// 観測 1 点を書く．
fn log_observation(run: &mut Run, unit_id: &str, t: u64) {
    run.log_event(
        "observation",
        &ObservationEvent {
            unit_id,
            t,
            t_unit: T_UNIT,
        },
    )
    .unwrap_or_else(|e| panic!("{unit_id} の t={t} の observation の記録に失敗: {e}"));
}

/// `events.jsonl` に書く終端行．
///
/// 先頭 6 フィールドは runvault の予約語 (`terminal` はこれを全部要求する)．
/// 残りは自由欄で，`phase` はここにしか置けない — 相はラベルであって数ではない．
#[derive(Serialize)]
struct TerminalEvent<'a> {
    unit_id: &'a str,
    t: u64,
    t_unit: &'static str,
    outcome: &'static str,
    censored: bool,
    budget: u64,
    seed: u64,
    phase: &'static str,
    n_surviving: usize,
    n_splits: usize,
    mean: f64,
    variance: f64,
    max_delta: f64,
}

/// シミュレーション 1 本を `terminal` イベントとして書く．
///
/// 打ち切り (`censored`) の行は `t == budget` でなければならない．ドライバは
/// `max|Δx| < tol` で停止し，止まらなければ `max_iterations` まで回すので，
/// 収束しなかった run は必ず上限に達している．この不変条件は runvault が
/// `log_event` の書き込み時に検査するので，ここでは二重に持たない．
///
/// `observed` はこの単位を観測した時刻の列で，終端の `t` を必ず含む．`run` は
/// 全ステップを観測して `metrics.csv` に残すので全ステップを，`sweep` は各試行の
/// 最終ステップしか見ないのでその 1 点だけを渡す．
pub fn log_terminal(
    run: &mut Run,
    unit_id: &str,
    seed: u64,
    max_iterations: usize,
    observed: impl IntoIterator<Item = u64>,
    result: &SimulationResult,
) {
    let last = result
        .metrics_history
        .last()
        .expect("metrics_history は t=0 を含む");

    for t in observed {
        log_observation(run, unit_id, t);
    }

    let event = TerminalEvent {
        unit_id,
        t: result.final_iteration as u64,
        t_unit: T_UNIT,
        outcome: if result.converged {
            "converged"
        } else {
            "unconverged"
        },
        censored: !result.converged,
        budget: max_iterations as u64,
        seed,
        phase: Phase::classify(last.n_surviving).label(),
        n_surviving: last.n_surviving,
        n_splits: last.n_splits,
        mean: last.mean,
        variance: last.variance,
        max_delta: last.max_delta,
    };
    run.log_event("terminal", &event)
        .unwrap_or_else(|e| panic!("{unit_id} の terminal イベントの記録に失敗: {e}"));
}

// ---------------------------------------------------------------------------
// 条件 1 点ぶんの集約 (sweep の子 run)
// ---------------------------------------------------------------------------

/// 1 つの ε で回した試行群の最終値．集約の材料になる．
pub struct TrialOutcome {
    /// 収束したか．
    pub converged: bool,
    /// 収束 (または打ち切り) した反復番号．
    pub final_iteration: usize,
    /// 最終ステップの生存意見数．
    pub n_surviving: usize,
    /// 最終ステップの平均意見．
    pub mean: f64,
    /// 最終ステップの意見の分散．
    pub variance: f64,
}

impl TrialOutcome {
    /// [`SimulationResult`] の最終ステップから取り出す．
    pub fn from_result(result: &SimulationResult) -> Self {
        let last = result
            .metrics_history
            .last()
            .expect("metrics_history は t=0 を含む");
        TrialOutcome {
            converged: result.converged,
            final_iteration: result.final_iteration,
            n_surviving: last.n_surviving,
            mean: last.mean,
            variance: last.variance,
        }
    }
}

/// 1 条件 (1 つの ε) を 1 つの値で表す指標．
///
/// 試行ごとの値は `events.jsonl` の担当なので，ここには集約しか書かない．試行
/// ごとの `n_surviving` を指標にすると (`run_uid`, `step`, `scope`, `name`) が
/// 重複するので，散らばりが要る図は `events.jsonl` から組み直す．
pub fn log_condition_summary(run: &mut Run, trials: &[TrialOutcome]) {
    let n = trials.len();
    assert!(n > 0, "試行が 1 本もありません");
    let n_f = n as f64;

    let n_converged = trials.iter().filter(|t| t.converged).count();
    let mean = |f: &dyn Fn(&TrialOutcome) -> f64| trials.iter().map(f).sum::<f64>() / n_f;

    run.log_metrics(
        "run",
        &[
            ("n_units", n_f),
            ("n_converged", n_converged as f64),
            ("convergence_rate", n_converged as f64 / n_f),
            ("mean_n_surviving", mean(&|t| t.n_surviving as f64)),
            ("mean_opinion_mean", mean(&|t| t.mean)),
            ("mean_opinion_variance", mean(&|t| t.variance)),
            ("mean_final_iteration", mean(&|t| t.final_iteration as f64)),
        ],
    )
    .expect("run スコープの指標の記録に失敗");
}

// ---------------------------------------------------------------------------
// シードの派生
// ---------------------------------------------------------------------------

/// 試行 1 本のシードを base seed から決定的に派生させる．
///
/// `master_seed` として記録するのは `base` の方で，実際に各試行が使うシードは
/// これで作る．`(base, eps, index)` が同じなら常に同じ値を返し，どれか 1 つでも
/// 違えば別の値になる — この性質が壊れると，記録した `master_seed` から run を
/// 組み直せなくなる．
pub fn trial_seed(base: u64, eps: f64, index: usize) -> u64 {
    socsim_core::derive_seed(base, &[eps.to_bits(), index as u64])
}

#[cfg(test)]
mod tests {
    use super::trial_seed;

    #[test]
    fn same_inputs_give_the_same_seed() {
        assert_eq!(trial_seed(42, 0.15, 3), trial_seed(42, 0.15, 3));
        for index in 0..8 {
            assert_eq!(
                trial_seed(2026, 0.05, index),
                trial_seed(2026, 0.05, index),
                "index={index} で再現しなかった"
            );
        }
    }

    #[test]
    fn each_coordinate_changes_the_seed() {
        let base = trial_seed(42, 0.15, 0);
        assert_ne!(base, trial_seed(43, 0.15, 0), "base が効いていない");
        assert_ne!(base, trial_seed(42, 0.20, 0), "eps が効いていない");
        assert_ne!(base, trial_seed(42, 0.15, 1), "index が効いていない");
    }

    #[test]
    fn one_condition_gives_distinct_seeds_across_trials() {
        let seeds: std::collections::BTreeSet<u64> =
            (0..64).map(|i| trial_seed(42, 0.15, i)).collect();
        assert_eq!(seeds.len(), 64, "同一条件の試行でシードが衝突した");
    }

    /// 具体値を固定する．
    ///
    /// ここが変わるのは socsim の `derive_seed` が変わったときで，そのときは
    /// 過去の run と結果を比較できなくなっている．Cargo.lock が socsim の commit を
    /// 固定しているので，この値は依存を上げたときにだけ動く．
    #[test]
    fn golden_values_are_pinned() {
        assert_eq!(trial_seed(42, 0.05, 0), 4_430_255_464_889_723_913);
        assert_eq!(trial_seed(42, 0.05, 1), 4_430_254_365_378_095_702);
        assert_eq!(trial_seed(42, 0.15, 2), 18_440_788_033_729_713_646);
    }
}
