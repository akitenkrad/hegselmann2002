//! socsim フレームワーク上の Bounded Confidence (BC) モデルの世界状態．
//!
//! `OpinionWorld` は socsim の [`WorldState`] を実装する非空間モデルである．
//! 意見は連続値 `x_i ∈ [0,1]` の 1 次元ベクトルで，空間占有もネットワーク位相
//! も持たない (正準モデルは完全グラフ; 論文 §4)．したがって `socsim-grid` /
//! `socsim-net` は不使用である．
//!
//! BC モデルの「近傍」(信頼集合) は固定位相ではなく，意見距離
//! `|x_i - x_j| ≤ ε` で毎ステップ動的に決まる完全グラフ上の部分集合である．
//! [`Neighbors`] は完全グラフとして「自分以外の全エージェント」を id 昇順で
//! 返し，ε による信頼集合の絞り込みはパックの `HegselmannKrauseMechanism`
//! 内で行われる．

use socsim_core::{AgentId, Neighbors, ScalarOpinions, SimClock, WorldState};

/// Bounded Confidence モデルの世界状態．
///
/// `eps_l` / `eps_r` は左右の信頼幅を保持するが，Phase 1 では `eps_l == eps_r`
/// (対称 BC) を前提とする．Phase 3 で独自 `AsymmetricHegselmannKrauseMechanism`
/// が両側を独立に参照する設計余地を残してある．
pub struct OpinionWorld {
    /// シミュレーションクロック．
    pub clock: SimClock,
    /// エージェント ID (`0..n`，ソート済み)．
    pub agents: Vec<AgentId>,
    /// 各エージェントの意見 `x_i(t) ∈ [0,1]`，index = agent_id．
    pub opinions: Vec<f64>,
    /// 左信頼幅 ε_l (Phase 1 では `eps_l == eps_r`)．
    pub eps_l: f64,
    /// 右信頼幅 ε_r (Phase 1 では `eps_l == eps_r`)．
    pub eps_r: f64,
}

impl OpinionWorld {
    /// 初期意見ベクトルから世界状態を構築する．
    ///
    /// Phase 1 の対称版は `eps_l == eps_r == eps` で呼び出す．
    pub fn new(opinions: Vec<f64>, eps_l: f64, eps_r: f64, t_max: u64) -> Self {
        let agents = (0..opinions.len() as u64).map(AgentId).collect();
        OpinionWorld {
            clock: SimClock::new(t_max),
            agents,
            opinions,
            eps_l,
            eps_r,
        }
    }

    /// エージェント数 n．
    pub fn n(&self) -> usize {
        self.opinions.len()
    }
}

impl WorldState for OpinionWorld {
    fn agent_ids(&self) -> Vec<AgentId> {
        // すでにソート済みだが，契約 (sorted) を明示するためそのまま返す．
        self.agents.clone()
    }

    fn clock(&self) -> &SimClock {
        &self.clock
    }

    fn clock_mut(&mut self) -> &mut SimClock {
        &mut self.clock
    }
}

impl ScalarOpinions for OpinionWorld {
    fn opinion(&self, id: AgentId) -> f64 {
        self.opinions[id.0 as usize]
    }

    fn set_opinion(&mut self, id: AgentId, value: f64) {
        self.opinions[id.0 as usize] = value;
    }
}

impl Neighbors for OpinionWorld {
    /// 完全グラフ・非空間モデルなので「近傍」は自分以外の全エージェント (id 昇順)．
    /// HK メカニズムが自分自身を信頼集合へ追加するため，ここでは自分を除く．
    fn neighbors_of(&self, id: AgentId) -> Vec<AgentId> {
        self.agents.iter().copied().filter(|&a| a != id).collect()
    }
}
