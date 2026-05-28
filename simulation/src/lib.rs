//! Hegselmann & Krause (2002) 有界信頼意見力学の再現実装ライブラリ．
//!
//! 論文 *Opinion Dynamics and Bounded Confidence: Models, Analysis and
//! Simulation* (JASSS 5(3), 2) の **Bounded Confidence (BC) モデル** を，
//! socsim フレームワーク上に構築した API として公開する．対象は連続的意見
//! $x_i \in [0,1]$ の同期的局所平均化 (`HegselmannKrauseMechanism` 流用) で，
//! 信頼幅 $\varepsilon$ の増大に伴う **断片化 → 分極 → 合意** の相転移を再現する．
//!
//! Phase 1 (対称 $\varepsilon_l = \varepsilon_r$ + `run`) と Phase 2 (`sweep` +
//! `visualize` / `visualize-sweep`) のみ実装している．Phase 3 (非対称
//! $\varepsilon_l \ne \varepsilon_r$ + 独自 `AsymmetricHegselmannKrauseMechanism`
//! + `reproduce` + socsim 上流 PR) は拡張点として配線・コメントだけ残してある．
//!
//! 設定構造体 (`config`)・世界状態 (`world`)・更新メカニズム再エクスポート
//! (`mechanisms`)・実行ドライバ (`simulation`)・集計メトリクス (`metrics`) を
//! モジュールとして公開し，バイナリ (`hegselmann-bc`) と統合テストの双方から
//! 利用する．

pub mod config;
pub mod mechanisms;
pub mod metrics;
pub mod simulation;
pub mod world;
