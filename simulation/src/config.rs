//! シミュレーション設定．
//!
//! 論文 (Hegselmann & Krause 2002) §4 の BC モデルを駆動するパラメータを
//! まとめる．Phase 1 は対称信頼幅 `eps_l == eps_r` を前提とするが，将来の
//! Phase 3 (非対称 $\varepsilon_l \ne \varepsilon_r$) に備えて両側のフィールド
//! を持つ．`Config::from_symmetric` は対称版のための便利コンストラクタである．

use serde::Serialize;

/// 初期意見プロファイルの生成方法．
///
/// 論文 Fig. 3 / 12 は一様乱数 (`Uniform`)，Fig. 4–8 は等間隔 (`Regular`)
/// `x_i = i / (n-1)` を用いる．
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartProfile {
    /// `[0,1]` 上の一様乱数．
    Uniform,
    /// 等間隔 `x_i = i / (n - 1)` (`n = 1` のときは 0.5)．
    Regular,
}

impl StartProfile {
    /// ラベル文字列を返す．`config.json` / CLI 出力で利用する．
    pub fn label(&self) -> &'static str {
        match self {
            StartProfile::Uniform => "uniform",
            StartProfile::Regular => "regular",
        }
    }
}

/// 文字列から [`StartProfile`] をパースする．
pub fn parse_start_profile(s: &str) -> Result<StartProfile, String> {
    match s.trim() {
        "uniform" => Ok(StartProfile::Uniform),
        "regular" => Ok(StartProfile::Regular),
        _ => Err(format!(
            "不正な初期分布: \"{}\" (uniform / regular のみ対応)",
            s
        )),
    }
}

/// 単一実行の設定．
///
/// `eps_l` と `eps_r` は左右の信頼幅で，Phase 1 では常に `eps_l == eps_r`
/// (対称 BC モデル) として扱う．Phase 3 では別々に指定する `--eps-l` /
/// `--eps-r` を CLI から受けて独自 mechanism に切替える予定．
#[derive(Debug, Clone)]
pub struct Config {
    /// エージェント数 n．
    pub n: usize,
    /// 左信頼幅 ε_l．Phase 1 では `eps_l == eps_r`．
    pub eps_l: f64,
    /// 右信頼幅 ε_r．Phase 1 では `eps_l == eps_r`．
    pub eps_r: f64,
    /// 初期意見プロファイル．
    pub start_profile: StartProfile,
    /// 最大反復回数 T．
    pub max_iterations: usize,
    /// 収束判定の許容誤差 (`max|Δx| < tol` で停止)．
    pub tol: f64,
    /// 乱数シード (`None` の場合はランダム)．
    pub seed: Option<u64>,
    /// 結果出力ディレクトリ．
    pub output_dir: String,
}

impl Default for Config {
    /// 論文 §4 に近い標準設定 (n=625, ε=0.15, uniform)．
    fn default() -> Self {
        Config {
            n: 625,
            eps_l: 0.15,
            eps_r: 0.15,
            start_profile: StartProfile::Uniform,
            max_iterations: 100,
            tol: 1e-6,
            seed: Some(42),
            output_dir: "results".to_string(),
        }
    }
}

impl Config {
    /// 対称版 (`eps_l == eps_r == eps`) を構築する便利コンストラクタ．
    ///
    /// Phase 1 / Phase 2 の標準経路はすべてこれを経由する．Phase 3 で非対称
    /// 信頼を扱う場合は [`Config`] のフィールドを直接設定する．
    pub fn from_symmetric(
        n: usize,
        eps: f64,
        start_profile: StartProfile,
        max_iterations: usize,
        tol: f64,
        seed: Option<u64>,
        output_dir: String,
    ) -> Self {
        Config {
            n,
            eps_l: eps,
            eps_r: eps,
            start_profile,
            max_iterations,
            tol,
            seed,
            output_dir,
        }
    }

    /// 対称信頼幅 (`eps_l == eps_r`) かどうか．
    ///
    /// Phase 1 のドライバ ([`crate::simulation::run`]) はこれを前提とする．
    pub fn is_symmetric(&self) -> bool {
        (self.eps_l - self.eps_r).abs() < f64::EPSILON
    }

    /// `config.json` 用の表現を組み立てる．
    pub fn to_run_config_json(&self) -> RunConfigJson {
        RunConfigJson {
            command: "run",
            n: self.n,
            eps_l: self.eps_l,
            eps_r: self.eps_r,
            symmetric: self.is_symmetric(),
            start_profile: self.start_profile.label(),
            max_iterations: self.max_iterations,
            tol: self.tol,
            seed: self.seed,
            output_dir: self.output_dir.clone(),
        }
    }
}

/// `config.json` (run 用) のシリアライズ表現．
#[derive(Serialize)]
pub struct RunConfigJson {
    pub command: &'static str,
    pub n: usize,
    pub eps_l: f64,
    pub eps_r: f64,
    /// `eps_l == eps_r` のとき `true`．Phase 1 では常に true．
    pub symmetric: bool,
    pub start_profile: &'static str,
    pub max_iterations: usize,
    pub tol: f64,
    pub seed: Option<u64>,
    pub output_dir: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_constructor_sets_both_sides() {
        let cfg = Config::from_symmetric(
            100,
            0.2,
            StartProfile::Uniform,
            50,
            1e-6,
            Some(1),
            "out".to_string(),
        );
        assert_eq!(cfg.eps_l, 0.2);
        assert_eq!(cfg.eps_r, 0.2);
        assert!(cfg.is_symmetric());
    }

    #[test]
    fn asymmetric_eps_is_detected() {
        let cfg = Config {
            eps_l: 0.05,
            eps_r: 0.25,
            ..Config::default()
        };
        assert!(!cfg.is_symmetric());
    }

    #[test]
    fn parse_start_profile_accepts_both() {
        assert_eq!(
            parse_start_profile("uniform").unwrap(),
            StartProfile::Uniform
        );
        assert_eq!(
            parse_start_profile("regular").unwrap(),
            StartProfile::Regular
        );
        assert!(parse_start_profile("zigzag").is_err());
    }
}
