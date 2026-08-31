//! Hegselmann & Krause (2002) "Opinion Dynamics and Bounded Confidence" — 再現実験の CLI．
//!
//! `run`   : 単一の (n, ε, 初期分布) で BC 力学を実行する．
//!           `--eps` (対称) と `--eps-l` / `--eps-r` (非対称 BC; 論文 §4.2 / Fig. 10–13)
//!           の両モードに対応．
//! `sweep` : ε を走査し，ε 1 点ごとに子 run を起こして `runs` 本の試行を回す．
//!           対称スイープのみ (Phase 2)．非対称スイープは Phase 3 reproduce 側で扱う余地．
//!
//! Phase 1 + Phase 2 + Phase 3 非対称 BC まで実装済み．論文 Figure 一括再現
//! (`reproduce`) のみ Phase 3 残作業．
//!
//! 出力の置き場と同一性は runvault が持つ．タイムスタンプ付きディレクトリも
//! `latest` シンボリックリンクもこちらでは作らず，`Run::start` が決めた run
//! ディレクトリへ書く．

use clap::{Parser, Subcommand};
use runvault::{Lineage, Run, RunOptions};
use serde::Serialize;

use hegselmann_bc_simulation::config::{parse_start_profile, Config};
use hegselmann_bc_simulation::metrics::Phase;
use hegselmann_bc_simulation::record::{self, DOMAIN, EXPERIMENT, REPO_ID};
use hegselmann_bc_simulation::simulation::{run, save_opinions};

// ---------------------------------------------------------------------------
// CLI 定義
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "hegselmann-bc",
    about = "Hegselmann & Krause (2002) Opinion Dynamics and Bounded Confidence — 再現実験 (Phase 1 + Phase 2 + Phase 3 非対称 BC)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 単一 (n, ε, 初期分布) で BC 力学を実行する．
    Run(RunArgs),
    /// ε を走査し，最終メトリクスを sweep_summary.csv に集計する．
    Sweep(SweepArgs),
}

#[derive(Parser, Debug)]
struct RunArgs {
    /// エージェント数 n．
    #[arg(long, default_value_t = 625)]
    n: usize,

    /// 対称信頼幅 ε．`--eps-l` / `--eps-r` 非指定時に両側に適用される．
    #[arg(long, default_value_t = 0.15)]
    eps: f64,

    /// 左信頼幅 ε_l (非対称 BC モード時のみ; --eps-r と必ずペア指定)．省略時は --eps を両側に適用 (対称)．
    #[arg(long)]
    eps_l: Option<f64>,

    /// 右信頼幅 ε_r (非対称 BC モード時のみ; --eps-l と必ずペア指定)．省略時は --eps を両側に適用 (対称)．
    #[arg(long)]
    eps_r: Option<f64>,

    /// 初期意見プロファイル (uniform | regular)．
    #[arg(long, default_value = "uniform")]
    start: String,

    /// 最大反復回数 T．
    #[arg(long, default_value_t = 100)]
    max_iterations: usize,

    /// 収束判定の許容誤差 (`max|Δx| < tol` で停止)．
    #[arg(long, default_value_t = 1e-6)]
    tol: f64,

    /// 乱数シード (省略時はランダム)．
    #[arg(long)]
    seed: Option<u64>,

    /// 結果出力ベースディレクトリ．
    #[arg(long, default_value = "results")]
    output_dir: String,
}

#[derive(Parser, Debug)]
struct SweepArgs {
    /// ε 走査の最小値．
    #[arg(long, default_value_t = 0.01)]
    eps_min: f64,

    /// ε 走査の最大値 (含む)．
    #[arg(long, default_value_t = 0.40)]
    eps_max: f64,

    /// ε 走査の刻み幅．
    #[arg(long, default_value_t = 0.01)]
    eps_step: f64,

    /// エージェント数 n．
    #[arg(long, default_value_t = 625)]
    n: usize,

    /// 初期意見プロファイル (uniform | regular)．
    #[arg(long, default_value = "uniform")]
    start: String,

    /// 各 ε あたりの独立試行数．
    #[arg(long, default_value_t = 50)]
    runs: usize,

    /// 最大反復回数 T．
    #[arg(long, default_value_t = 100)]
    max_iterations: usize,

    /// 収束判定の許容誤差．
    #[arg(long, default_value_t = 1e-6)]
    tol: f64,

    /// 乱数シード基点 (各試行は derive により独立化する)．
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// 結果出力ベースディレクトリ．
    #[arg(long, default_value = "results")]
    output_dir: String,
}

// ---------------------------------------------------------------------------
// 補助
// ---------------------------------------------------------------------------

/// 小数点以下の桁数を文字列表現から推定する．
fn step_decimals(v: f64) -> usize {
    let s = format!("{}", v);
    match s.find('.') {
        Some(pos) => s.len() - pos - 1,
        None => 0,
    }
}

/// `eps_min..=eps_max` を `eps_step` 刻みの等差数列に展開する (浮動小数点誤差を丸める)．
fn eps_range(eps_min: f64, eps_max: f64, eps_step: f64) -> Vec<f64> {
    assert!(eps_step > 0.0, "eps-step は正でなければなりません");
    let n_steps = ((eps_max - eps_min) / eps_step + 0.5e-9).floor() as usize;
    let decimals = step_decimals(eps_step);
    let factor = 10_f64.powi(decimals as i32);
    (0..=n_steps)
        .map(|i| ((eps_min + eps_step * i as f64) * factor).round() / factor)
        .collect()
}

/// スイープ親 run の実験条件 (グリッド定義そのもの)．
#[derive(Serialize)]
struct SweepParameters {
    eps_min: f64,
    eps_max: f64,
    eps_step: f64,
    n: usize,
    runs: usize,
    max_iterations: usize,
    tol: f64,
    seed: u64,
    start_profile: &'static str,
}

/// スイープの子 run (ε 1 点) の実験条件．
///
/// `run` の条件に `runs` が付いた形で，`run` とは別のサブコマンド名を持つ．
/// 同じ `run` を名乗らせると，「1 本のシミュレーション」と「同一条件の
/// `runs` 本」という中身の違う 2 つが 1 つの名前に同居し，`runvault path
/// --subcommand run` がどちらを返すか分からなくなる．
#[derive(Serialize)]
struct SweepPointParameters {
    n: usize,
    eps_l: f64,
    eps_r: f64,
    symmetric: bool,
    start_profile: &'static str,
    runs: usize,
    max_iterations: usize,
    tol: f64,
    seed: u64,
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

fn cmd_run(args: RunArgs) {
    let start_profile = parse_start_profile(&args.start).unwrap_or_else(|e| panic!("{}", e));

    // シードを実体化してから記録する．--seed 省略時にシミュレーション側で
    // rand::random に落とすと，実際に使われたシードがどこにも残らない．
    let seed = args.seed.unwrap_or_else(rand::random::<u64>);

    // 出力先は Run::start が run ディレクトリを決めた後に確定する．
    let mut cfg = match (args.eps_l, args.eps_r) {
        (None, None) => Config::from_symmetric(
            args.n,
            args.eps,
            start_profile,
            args.max_iterations,
            args.tol,
            Some(seed),
            String::new(),
        ),
        (Some(l), Some(r)) => Config::from_asymmetric(
            args.n,
            l,
            r,
            start_profile,
            args.max_iterations,
            args.tol,
            Some(seed),
            String::new(),
        ),
        _ => panic!("--eps-l と --eps-r は必ずペアで指定してください"),
    };

    let parameters = cfg.to_parameters(seed);
    let mut rv = Run::start(
        RunOptions::new(EXPERIMENT, "run")
            .repo_id(REPO_ID)
            .domain(DOMAIN)
            .results_root(&args.output_dir)
            .parameters(&parameters)
            .expect("runvault: parameters の組み立てに失敗")
            .seed_pointers(["/seed"])
            .master_seed(seed)
            .replication(record::replication()),
    )
    .expect("runvault: run の開始に失敗");

    // run ディレクトリが出力先そのものになる．意見の軌跡は artifacts/ の下へ．
    cfg.output_dir = rv.dir().join("artifacts").to_string_lossy().into_owned();

    println!("=== Hegselmann–Krause (2002) BC 力学 再現実験 ===");
    if cfg.is_symmetric() {
        println!(
            "n: {} | ε: {} | 初期分布: {} | max_iter: {} | tol: {}",
            cfg.n,
            cfg.eps_l,
            start_profile.label(),
            cfg.max_iterations,
            cfg.tol,
        );
    } else {
        println!(
            "n: {} | ε_l/ε_r: {} / {} | 初期分布: {} | max_iter: {} | tol: {}",
            cfg.n,
            cfg.eps_l,
            cfg.eps_r,
            start_profile.label(),
            cfg.max_iterations,
            cfg.tol,
        );
    }
    println!("シード: {}", seed);
    println!("出力先: {}", rv.dir().display());
    println!("-------------------------------------------");

    let result = run(&cfg);
    save_opinions(&result.opinion_history, &cfg.output_dir);
    record::log_simulation(&mut rv, &result);
    // run は全ステップを観測して metrics.csv に残しているので，観測時刻も全ステップ．
    let observed: Vec<u64> = result.metrics_history.iter().map(|m| m.t as u64).collect();
    record::log_terminal(&mut rv, "run", seed, cfg.max_iterations, observed, &result);

    let last = result.metrics_history.last().unwrap();
    let phase = Phase::classify(last.n_surviving);
    println!(
        "収束: {} | 反復回数: {}",
        if result.converged { "Yes" } else { "No" },
        result.final_iteration,
    );
    println!(
        "生存意見数: {} | 分裂数: {} | 相: {} | 平均意見: {:.4} | 分散: {:.4e}",
        last.n_surviving,
        last.n_splits,
        phase.label(),
        last.mean,
        last.variance,
    );

    let dir = rv.finish().expect("runvault: run の完了に失敗");
    println!("意見軌跡 → {}/artifacts/opinions.csv", dir.display());
    println!("メトリクス → {}/metrics.csv", dir.display());
    println!("終端の相   → {}/events.jsonl", dir.display());
    println!("設定       → {}/config.json", dir.display());
}

// ---------------------------------------------------------------------------
// sweep
// ---------------------------------------------------------------------------

fn cmd_sweep(args: SweepArgs) {
    let start_profile = parse_start_profile(&args.start).unwrap_or_else(|e| panic!("{}", e));

    let epss = eps_range(args.eps_min, args.eps_max, args.eps_step);
    let n_total = epss.len() * args.runs;

    let sweep_parameters = SweepParameters {
        eps_min: args.eps_min,
        eps_max: args.eps_max,
        eps_step: args.eps_step,
        n: args.n,
        runs: args.runs,
        max_iterations: args.max_iterations,
        tol: args.tol,
        seed: args.seed,
        start_profile: start_profile.label(),
    };

    // 親 run: ε のグリッド定義そのものを parameters に持つ．個別条件の指標は
    // 書かない．親は 1 本のシミュレーションではないので master_seed を名乗らず，
    // base seed は /parameters.seed と seed_pointers 経由で execution_hash に残る．
    // sweep_id は runvault が親の run_slug で埋める．
    let parent = Run::start(
        RunOptions::new(EXPERIMENT, "sweep")
            .repo_id(REPO_ID)
            .domain(DOMAIN)
            .results_root(&args.output_dir)
            .parameters(&sweep_parameters)
            .expect("runvault: sweep の parameters の組み立てに失敗")
            .seed_pointers(["/seed"])
            .sweep_parent()
            .replication(record::replication()),
    )
    .expect("runvault: sweep 親 run の開始に失敗");

    let sweep_id = parent
        .sweep_id()
        .expect("runvault: sweep 親に sweep_id がありません")
        .to_string();
    let parent_run_uid = parent.run_uid().to_string();

    println!("=== Hegselmann–Krause (2002) BC 力学 パラメータスイープ ===");
    println!(
        "n: {} | ε: {} 値 ({}..={}, step {}) | 試行: {} | 初期分布: {} | 合計: {} 実行",
        args.n,
        epss.len(),
        args.eps_min,
        args.eps_max,
        args.eps_step,
        args.runs,
        start_profile.label(),
        n_total,
    );
    println!("シード (base): {}", args.seed);
    println!("出力先: {}", parent.dir().display());
    println!("---------------------------------------------------");

    let mut done = 0usize;

    for &eps in &epss {
        let params = SweepPointParameters {
            n: args.n,
            eps_l: eps,
            eps_r: eps,
            symmetric: true,
            start_profile: start_profile.label(),
            runs: args.runs,
            max_iterations: args.max_iterations,
            tol: args.tol,
            seed: args.seed,
        };

        // 子は「その ε の試行群」そのもの．master_seed は親と同じ base で，
        // ε が違えば config_hash が違うので run としては別物になる．
        // 同じ条件の繰り返しは無いので replicate_index は 0．
        let mut child = Run::start(
            RunOptions::new(EXPERIMENT, "sweep-point")
                .repo_id(REPO_ID)
                .domain(DOMAIN)
                .results_root(&args.output_dir)
                .parameters(&params)
                .expect("runvault: 子 run の parameters の組み立てに失敗")
                .seed_pointers(["/seed"])
                .master_seed(args.seed)
                .replicate_index(0)
                .lineage(Lineage {
                    sweep_id: Some(sweep_id.clone()),
                    parent_run_uid: Some(parent_run_uid.clone()),
                    ..Default::default()
                })
                .replication(record::replication()),
        )
        .expect("runvault: 子 run の開始に失敗");

        let mut trials: Vec<record::TrialOutcome> = Vec::with_capacity(args.runs);
        for run_idx in 0..args.runs {
            // 各 (eps, run) に独立なシードを派生させる (explicit identity)．
            let seed = record::trial_seed(args.seed, eps, run_idx);

            let cfg = Config::from_symmetric(
                args.n,
                eps,
                start_profile,
                args.max_iterations,
                args.tol,
                Some(seed),
                String::new(),
            );

            let result = run(&cfg);
            // sweep が見るのは各試行の最終ステップだけなので，観測時刻もそこ 1 点．
            record::log_terminal(
                &mut child,
                &format!("trial-{run_idx}"),
                seed,
                args.max_iterations,
                [result.final_iteration as u64],
                &result,
            );
            trials.push(record::TrialOutcome::from_result(&result));

            done += 1;
        }
        record::log_condition_summary(&mut child, &trials);

        let n_converged = trials.iter().filter(|t| t.converged).count();
        let mean_n_surviving =
            trials.iter().map(|t| t.n_surviving as f64).sum::<f64>() / trials.len() as f64;
        println!(
            "[{}/{}] ε={:.4} 完了 ({} 試行) → converged={}/{} mean_n_surviving={:.2}",
            done,
            n_total,
            eps,
            args.runs,
            n_converged,
            trials.len(),
            mean_n_surviving,
        );

        child.finish().expect("runvault: 子 run の完了に失敗");
    }

    let dir = parent
        .finish()
        .expect("runvault: sweep 親 run の完了に失敗");
    println!("===================================================");
    println!("スイープ完了: {} 実行", n_total);
    println!("スイープ定義 → {}/config.json", dir.display());
    println!("各 ε の試行は子 run (subcommand=sweep-point) の events.jsonl にあります");
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run(args) => cmd_run(args),
        Commands::Sweep(args) => cmd_sweep(args),
    }
}
