//! Hegselmann & Krause (2002) "Opinion Dynamics and Bounded Confidence" — 再現実験の CLI．
//!
//! `run`   : 単一の (n, ε, 初期分布) で BC 力学を実行する．
//!           `--eps` (対称) と `--eps-l` / `--eps-r` (非対称 BC; 論文 §4.2 / Fig. 10–13)
//!           の両モードに対応．
//! `sweep` : ε を走査し，各 (ε, run) で最終メトリクスを `sweep_summary.csv` に集計する．
//!           対称スイープのみ (Phase 2)．非対称スイープは Phase 3 reproduce 側で扱う余地．
//!
//! Phase 1 + Phase 2 + Phase 3 非対称 BC まで実装済み．論文 Figure 一括再現
//! (`reproduce`) のみ Phase 3 残作業．

use clap::{Parser, Subcommand};
use serde::Serialize;
use socsim_results::{ensure_dir, refresh_latest_symlink, timestamp, write_csv, write_json};

use hegselmann_bc_simulation::config::{parse_start_profile, Config};
use hegselmann_bc_simulation::metrics::Phase;
use hegselmann_bc_simulation::simulation::{ensure_output_dir, run, save_metrics, save_opinions};

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

/// `sweep_summary.csv` の 1 行 ((eps, run) → 最終メトリクス)．
#[derive(Serialize)]
struct SweepRow {
    eps: f64,
    run_id: usize,
    seed: u64,
    converged: bool,
    final_iteration: usize,
    n_surviving: usize,
    mean: f64,
    variance: f64,
    n_splits: usize,
    phase: u8,
    max_delta: f64,
}

/// `sweep_config.json` の構造体．
#[derive(Serialize)]
struct SweepConfigJson {
    command: &'static str,
    eps_min: f64,
    eps_max: f64,
    eps_step: f64,
    n: usize,
    runs: usize,
    max_iterations: usize,
    tol: f64,
    seed: u64,
    start_profile: String,
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

fn cmd_run(args: RunArgs) {
    let start_profile = parse_start_profile(&args.start).unwrap_or_else(|e| panic!("{}", e));

    let timestamp = timestamp();
    let output_dir = format!("{}/{}", args.output_dir, timestamp);

    let cfg = match (args.eps_l, args.eps_r) {
        (None, None) => Config::from_symmetric(
            args.n,
            args.eps,
            start_profile,
            args.max_iterations,
            args.tol,
            args.seed,
            output_dir.clone(),
        ),
        (Some(l), Some(r)) => Config::from_asymmetric(
            args.n,
            l,
            r,
            start_profile,
            args.max_iterations,
            args.tol,
            args.seed,
            output_dir.clone(),
        ),
        _ => panic!("--eps-l と --eps-r は必ずペアで指定してください"),
    };

    ensure_output_dir(&cfg.output_dir);

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
    println!("シード: {:?}", cfg.seed);
    println!("出力先: {}", cfg.output_dir);
    println!("-------------------------------------------");

    let result = run(&cfg);
    save_metrics(&result.metrics_history, &cfg.output_dir);
    save_opinions(&result.opinion_history, &cfg.output_dir);

    // config.json (pretty-print JSON; socsim_results::write_json に委譲)．
    let cfg_path = format!("{}/config.json", cfg.output_dir);
    write_json(&cfg.to_run_config_json(), &cfg_path).expect("config.json の書き込みに失敗");

    // latest シンボリックリンクを再作成する (best-effort; 失敗しても無視)．
    let _ = refresh_latest_symlink(&args.output_dir, &timestamp);

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
    println!("意見軌跡 → {}/opinions.csv", cfg.output_dir);
    println!("メトリクス → {}/metrics.csv", cfg.output_dir);
    println!("設定       → {}/config.json", cfg.output_dir);
}

// ---------------------------------------------------------------------------
// sweep
// ---------------------------------------------------------------------------

fn cmd_sweep(args: SweepArgs) {
    let start_profile = parse_start_profile(&args.start).unwrap_or_else(|e| panic!("{}", e));

    let epss = eps_range(args.eps_min, args.eps_max, args.eps_step);

    let timestamp = timestamp();
    let sweep_dir = format!("{}/{}_sweep", args.output_dir, timestamp);
    ensure_dir(&sweep_dir).expect("sweep ディレクトリの作成に失敗");

    let n_total = epss.len() * args.runs;

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
    println!("出力先: {}", sweep_dir);
    println!("---------------------------------------------------");

    let mut summary_rows: Vec<SweepRow> = Vec::with_capacity(n_total);
    let mut done = 0usize;

    for &eps in &epss {
        for run_idx in 0..args.runs {
            // 各 (eps, run) に独立なシードを派生させる (explicit identity)．
            let seed = socsim_core::derive_seed(args.seed, &[eps.to_bits(), run_idx as u64]);

            let cfg = Config::from_symmetric(
                args.n,
                eps,
                start_profile,
                args.max_iterations,
                args.tol,
                Some(seed),
                sweep_dir.clone(),
            );

            let result = run(&cfg);
            let last = result.metrics_history.last().unwrap();

            summary_rows.push(SweepRow {
                eps,
                run_id: run_idx,
                seed,
                converged: result.converged,
                final_iteration: result.final_iteration,
                n_surviving: last.n_surviving,
                mean: last.mean,
                variance: last.variance,
                n_splits: last.n_splits,
                phase: last.phase,
                max_delta: last.max_delta,
            });

            done += 1;
        }
        println!(
            "[{}/{}] ε={:.4} 完了 ({} 試行)",
            done, n_total, eps, args.runs,
        );
    }

    // sweep_summary.csv
    let summary_path = format!("{}/sweep_summary.csv", sweep_dir);
    write_csv(&summary_rows, &summary_path).expect("sweep_summary.csv の書き込みに失敗");

    // sweep_config.json
    let config_json = SweepConfigJson {
        command: "sweep",
        eps_min: args.eps_min,
        eps_max: args.eps_max,
        eps_step: args.eps_step,
        n: args.n,
        runs: args.runs,
        max_iterations: args.max_iterations,
        tol: args.tol,
        seed: args.seed,
        start_profile: start_profile.label().to_string(),
    };
    let cfg_path = format!("{}/sweep_config.json", sweep_dir);
    write_json(&config_json, &cfg_path).expect("sweep_config.json の書き込みに失敗");

    let _ = refresh_latest_symlink(&args.output_dir, &format!("{}_sweep", timestamp));

    println!("===================================================");
    println!("スイープ完了: {} 実行", n_total);
    println!("サマリ → {}", summary_path);
    println!("設定   → {}", cfg_path);
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
