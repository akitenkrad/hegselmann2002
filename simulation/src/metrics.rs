//! 評価指標．
//!
//! 論文 (Hegselmann & Krause 2002) §4 のベンチマークに対応する指標を計算する．
//! 中心は **生存意見数** (`n_surviving`) と **相分類** (`Phase`) — 信頼幅 ε の
//! 増大による断片化 → 分極 → 合意の三相転移を定量化する．
//!
//! ## 流用と独自実装の境界
//!
//! - 流用: 平均 / 分散 / 生存クラスタ数は `socsim-metrics::stats` の
//!   [`mean`](socsim_metrics::stats::mean) /
//!   [`variance`](socsim_metrics::stats::variance) /
//!   [`distinct_clusters`](socsim_metrics::stats::distinct_clusters) を直接使う．
//! - 独自: 連続区間の分裂数 (`n_splits`; 論文 Fig. 7「8 splits」) と相分類
//!   (`Phase`; 論文 Fig. 3 の相転移) は本論文に固有の意味付けがあるため
//!   ローカル実装に留める (設計書 §4.3 評価指標 表)．

use socsim_metrics::stats::{distinct_clusters, mean as stats_mean, variance as stats_variance};

/// 生存意見クラスタの結合許容誤差 (論文の "8 splits" 等の解像度に合わせる)．
///
/// `distinct_clusters` がソート列の隣接ペアの絶対差 `> CLUSTER_TOL` を分裂とみなす
/// ため，`1e-4` 刻みで連続区間を 1 クラスタにまとめる．`n_surviving` と
/// `n_splits` の両方で同じ閾値を使うことで，両指標が同じ「クラスタ感覚」を
/// 共有する．
pub const CLUSTER_TOL: f64 = 1e-4;

/// 安定後の相 (phase) 分類．生存意見数で判定する．
///
/// 論文 §4 では `ε` の増大に伴い `Plurality (>10)` → `Polarization (2..=10)`
/// → `Consensus (1)` という三相転移が観測される．閾値は本リポジトリの実装
/// 都合 (sweep 集計で読みやすい区分) で，論文には明示数値はない．
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 合意 (consensus): 生存意見数 ≤ 1．
    Consensus,
    /// 分極 (polarization): 生存意見数 2..=10．
    Polarization,
    /// 多元 (plurality): 生存意見数 > 10．
    Plurality,
}

impl Phase {
    /// 生存意見数から相を分類する．
    pub fn classify(n_surviving: usize) -> Phase {
        match n_surviving {
            0 | 1 => Phase::Consensus,
            2..=10 => Phase::Polarization,
            _ => Phase::Plurality,
        }
    }

    /// CSV/JSON 用の整数コード (consensus=1 / polarization=2 / plurality=3)．
    pub fn code(&self) -> u8 {
        match self {
            Phase::Consensus => 1,
            Phase::Polarization => 2,
            Phase::Plurality => 3,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Phase::Consensus => "consensus",
            Phase::Polarization => "polarization",
            Phase::Plurality => "plurality",
        }
    }
}

/// 生存意見数 = 連続クラスタ数．
///
/// `socsim_metrics::stats::distinct_clusters` (ソート列の隣接ギャップ
/// `> tol` を分裂とみなす greedy single-linkage) を [`CLUSTER_TOL`] 解像度で
/// 呼ぶ．論文 §4 の "surviving opinions" / Fig. 12 の "number of opinions
/// at convergence" に対応する．
pub fn n_surviving(opinions: &[f64]) -> usize {
    distinct_clusters(opinions, CLUSTER_TOL)
}

/// 連続区間の分裂数 = `n_surviving - 1`．
///
/// 論文 Fig. 7 では n=100 の regular 初期分布 + ε=0.05 で「8 splits」(= 9
/// クラスタ) が観測される．意味付け: 連続な ε-profile 上の隣接ペアのうち
/// ギャップ `> ε` のものが何箇所あるか．
pub fn n_splits(opinions: &[f64]) -> usize {
    n_surviving(opinions).saturating_sub(1)
}

/// 1 ステップ分のメトリクス．
///
/// runvault の `metrics.csv` は long 形式なので，この構造体がそのまま 1 行に
/// なるわけではない．`crate::record::log_step` が数値フィールドを名前つきの
/// 指標へ展開する ([`Metrics::phase`] だけは category なので指標にならず，
/// 終端イベントのラベルとして書かれる)．
#[derive(Debug, Clone)]
pub struct Metrics {
    /// ステップ番号 t．
    pub t: usize,
    /// 生存意見数 (連続クラスタ数)．
    pub n_surviving: usize,
    /// 平均意見．
    pub mean: f64,
    /// 意見の分散 (population variance)．
    pub variance: f64,
    /// 連続区間の分裂数 (`= n_surviving - 1`)．
    pub n_splits: usize,
    /// 相コード (consensus=1 / polarization=2 / plurality=3)．
    pub phase: u8,
    /// 直近ステップの `max|Δx|` (step 0 では 0.0)．
    pub max_delta: f64,
}

impl Metrics {
    /// 意見ベクトルからメトリクスを計算する．
    ///
    /// `tol` は受け取るが現状は使わない (`n_surviving` は [`CLUSTER_TOL`] を
    /// 使う方針)．将来「クラスタ解像度を実行時に変えたい」需要が出たら本引数
    /// 経由で `distinct_clusters(opinions, tol)` に渡す拡張余地として残す．
    pub fn compute(opinions: &[f64], t: usize, max_delta: f64, _tol: f64) -> Self {
        let n_surv = n_surviving(opinions);
        Metrics {
            t,
            n_surviving: n_surv,
            mean: stats_mean(opinions),
            variance: stats_variance(opinions),
            n_splits: n_surv.saturating_sub(1),
            phase: Phase::classify(n_surv).code(),
            max_delta,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_cluster_is_one_surviving() {
        let v = vec![0.5, 0.5000001, 0.4999999];
        assert_eq!(n_surviving(&v), 1);
        assert_eq!(n_splits(&v), 0);
    }

    #[test]
    fn three_distant_opinions_count_as_three() {
        let v = vec![0.1, 0.5, 0.9];
        assert_eq!(n_surviving(&v), 3);
        assert_eq!(n_splits(&v), 2);
    }

    #[test]
    fn phase_thresholds() {
        assert_eq!(Phase::classify(1), Phase::Consensus);
        assert_eq!(Phase::classify(2), Phase::Polarization);
        assert_eq!(Phase::classify(10), Phase::Polarization);
        assert_eq!(Phase::classify(11), Phase::Plurality);
    }

    #[test]
    fn metrics_compute_basic() {
        let v = vec![0.1, 0.5, 0.9];
        let m = Metrics::compute(&v, 7, 0.01, 1e-6);
        assert_eq!(m.t, 7);
        assert_eq!(m.n_surviving, 3);
        assert_eq!(m.n_splits, 2);
        assert!((m.mean - 0.5).abs() < 1e-12);
        // 生存意見 3 は Phase::Polarization (2..=10) に分類される．
        assert_eq!(m.phase, Phase::Polarization.code());
        assert!((m.max_delta - 0.01).abs() < 1e-12);
    }
}
