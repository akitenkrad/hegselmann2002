//! 有界信頼更新メカニズム (socsim-mechanisms パックへ移譲)．
//!
//! Phase 1 (対称 BC モデル) の更新規則 (論文 §4 式 (BC))
//!
//! ```text
//! x_i(t+1) = (1 / |I(i, x(t))|) Σ_{j ∈ I(i, x(t))} x_j(t)
//!            I(i, x) = { j : |x_i − x_j| ≤ ε } ∪ {i}
//! ```
//!
//! は `socsim-mechanisms` パックの [`HegselmannKrauseMechanism`] を
//! `MeanOperator::Arithmetic` で構築すればそのまま満たされる．本リポジトリは
//! 自前 `BoundedConfidenceUpdate` を書かず，当該パック実装を再エクスポート
//! して利用する (姉妹実装 `hegselmann2005` と同様)．
//!
//! 収束判定 (`max|Δx| < tol` での停止) はパックの [`ConvergenceMechanism`] を
//! `PostStep` フェーズに配線する．BC モデルは決定論的なので有限時間で収束する
//! (論文 §3 Result 5)．
//!
//! ## Phase 3 拡張点 (非対称信頼)
//!
//! 非対称信頼 `ε_l ≠ ε_r` の信頼集合
//!
//! ```text
//! I(i, x) = { j : −ε_l ≤ x_j − x_i ≤ ε_r } ∪ {i}
//! ```
//!
//! は socsim-mechanisms PR #47 で [`HegselmannKrauseMechanism`] 自身が
//! [`HegselmannKrauseMechanism::with_asymmetric`] コンストラクタとして
//! サポートするようになったため，本リポジトリでは独自 mechanism を実装する
//! 必要は無い．対称 / 非対称の選択は [`crate::simulation::run`] が
//! [`crate::config::Config::is_symmetric`] に応じて行う．

pub use socsim_mechanisms::{ConvergenceMechanism, HegselmannKrauseMechanism};
