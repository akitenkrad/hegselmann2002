#!/usr/bin/env python3
"""
visualize.py — Hegselmann & Krause (2002) BC モデル 単一実行結果の可視化．

results/latest (または --results-dir 指定先) の opinions.csv / metrics.csv を読み，
時間×意見の軌跡図 (論文 Fig. 2 / 7 / 8 風) と，メトリクス時系列 (生存意見数 /
分散 / max|Δx|) を生成する．

Usage:
    uv run hegselmann-bc-tools visualize
    uv run hegselmann-bc-tools visualize --results-dir results/20260528_153000
    uv run hegselmann-bc-tools visualize --output-dir out
"""

from __future__ import annotations

import argparse
import os

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

# --------------------------------------------------------------------------- #
# 表示設定 (CJK フォントが利用不能でも落ちないように try)
# --------------------------------------------------------------------------- #
try:
    plt.rcParams["font.family"] = "Hiragino Sans"
except Exception:  # pragma: no cover - フォント未インストール環境用フォールバック
    pass

COLOR_BG = "#FAFAF8"
COLOR_TRAJ = "#2196F3"
COLOR_CLUSTER = "#534AB7"
COLOR_NSURV = "#F44336"
COLOR_VAR = "#9C27B0"
COLOR_DELTA = "#FF9800"


# --------------------------------------------------------------------------- #
# データ読み込み
# --------------------------------------------------------------------------- #

def load_opinions(path: str) -> pd.DataFrame:
    if not os.path.exists(path):
        raise FileNotFoundError(f"opinions.csv が見つかりません: {path}")
    return pd.read_csv(path)


def load_metrics(path: str) -> pd.DataFrame:
    if not os.path.exists(path):
        raise FileNotFoundError(f"metrics.csv が見つかりません: {path}")
    return pd.read_csv(path)


def to_wide(df_long: pd.DataFrame) -> tuple[np.ndarray, np.ndarray]:
    """long-format (t, agent_id, opinion) を (ts, [T x N] 行列) に変換する．"""
    pivot = df_long.pivot(index="t", columns="agent_id", values="opinion").sort_index()
    return pivot.index.to_numpy(), pivot.to_numpy()


# --------------------------------------------------------------------------- #
# 可視化
# --------------------------------------------------------------------------- #

def save_opinion_trajectory(
    ts: np.ndarray,
    mat: np.ndarray,
    df_metrics: pd.DataFrame,
    out_path: str,
    subtitle: str = "",
) -> None:
    """時間×意見の軌跡図 (論文 Fig. 2/7/8 風)．

    エージェントごとに半透明の細線を引き，最終ステップの生存クラスタ位置 (意見の
    平均座標) を太い水平線で重ねて強調する．
    """
    n_agents = mat.shape[1]
    fig, ax = plt.subplots(figsize=(9, 6), facecolor=COLOR_BG)
    ax.set_facecolor(COLOR_BG)

    alpha = max(0.05, min(0.8, 30.0 / max(n_agents, 1)))
    lw = 0.6 if n_agents > 200 else 1.0

    for i in range(n_agents):
        ax.plot(ts, mat[:, i], color=COLOR_TRAJ, alpha=alpha, lw=lw)

    # 最終意見からクラスタ中心 (~1e-4 解像度で連続値をビン化) を抽出して水平線を引く．
    final = np.sort(mat[-1])
    cluster_centers: list[float] = []
    if final.size > 0:
        current_bucket = [final[0]]
        for x in final[1:]:
            if abs(x - current_bucket[-1]) <= 1e-3:
                current_bucket.append(x)
            else:
                cluster_centers.append(float(np.mean(current_bucket)))
                current_bucket = [x]
        cluster_centers.append(float(np.mean(current_bucket)))

    for c in cluster_centers:
        ax.axhline(c, color=COLOR_CLUSTER, lw=1.2, alpha=0.55, linestyle="--")

    n_surv = int(df_metrics.iloc[-1]["n_surviving"]) if len(df_metrics) else 0

    ax.set_xlabel("時刻 t")
    ax.set_ylabel("意見 x ∈ [0, 1]")
    ax.set_ylim(-0.02, 1.02)
    ax.set_xlim(ts.min(), ts.max())
    title = f"意見軌跡 (Hegselmann–Krause 2002 BC モデル; 生存意見数 = {n_surv})"
    if subtitle:
        title += f"\n{subtitle}"
    ax.set_title(title, fontsize=12)
    ax.grid(True, alpha=0.3)

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


def save_metrics_timeseries(df: pd.DataFrame, out_path: str) -> None:
    """生存意見数・分散・max|Δx| の時系列を 3 段で表示する．"""
    fig, axes = plt.subplots(1, 3, figsize=(15, 4.5), facecolor=COLOR_BG)
    fig.suptitle("Hegselmann–Krause (2002) BC 力学 — メトリクス時系列", fontsize=13)

    t = df["t"]

    ax = axes[0]
    ax.set_facecolor(COLOR_BG)
    ax.plot(t, df["n_surviving"], color=COLOR_NSURV, lw=2)
    ax.set_xlabel("時刻 t")
    ax.set_ylabel("生存意見数")
    ax.set_title("生存意見数 (連続クラスタ)")
    ax.set_yscale("log")
    ax.grid(True, alpha=0.3, which="both")

    ax = axes[1]
    ax.set_facecolor(COLOR_BG)
    ax.plot(t, df["variance"], color=COLOR_VAR, lw=2)
    ax.set_xlabel("時刻 t")
    ax.set_ylabel("分散")
    ax.set_title("意見の分散")
    ax.grid(True, alpha=0.3)

    ax = axes[2]
    ax.set_facecolor(COLOR_BG)
    delta = df["max_delta"].clip(lower=1e-16)
    ax.plot(t, delta, color=COLOR_DELTA, lw=2)
    ax.set_xlabel("時刻 t")
    ax.set_ylabel("max|Δx|")
    ax.set_title("最大意見変化量 (収束指標)")
    ax.set_yscale("log")
    ax.grid(True, alpha=0.3, which="both")

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


# --------------------------------------------------------------------------- #
# メイン
# --------------------------------------------------------------------------- #

def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="hegselmann-bc-tools visualize",
        description="Hegselmann & Krause (2002) BC モデル 意見軌跡 可視化",
    )
    p.add_argument(
        "--results-dir", "--results_dir", default="results/latest",
        help="Rust シミュレーションの出力ディレクトリ (default: results/latest)",
    )
    p.add_argument(
        "--output-dir", "--output_dir", default=None,
        help="図の保存先ディレクトリ (default: {results-dir}/figures)",
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)

    opinions_path = os.path.join(args.results_dir, "opinions.csv")
    metrics_path = os.path.join(args.results_dir, "metrics.csv")
    out_dir = args.output_dir if args.output_dir else os.path.join(args.results_dir, "figures")

    os.makedirs(out_dir, exist_ok=True)

    print("=== Hegselmann–Krause (2002) BC 力学 可視化 ===")
    print(f"意見軌跡:   {opinions_path}")
    print(f"メトリクス: {metrics_path}")
    print(f"出力先:     {out_dir}")
    print("-----------------------------------------")

    print("[1/3] 意見軌跡を読み込み中 ...")
    df_op = load_opinions(opinions_path)
    ts, mat = to_wide(df_op)
    print(f"      {mat.shape[0]} ステップ × {mat.shape[1]} エージェント")

    print("[2/3] メトリクスを読み込み中 ...")
    df_m = load_metrics(metrics_path)

    print("[3/3] 図を保存中 ...")
    save_opinion_trajectory(
        ts, mat, df_m,
        os.path.join(out_dir, "opinion_trajectory.png"),
        subtitle=f"{mat.shape[1]} エージェント，{mat.shape[0] - 1} ステップ",
    )
    save_metrics_timeseries(df_m, os.path.join(out_dir, "metrics_timeseries.png"))

    print("-----------------------------------------")
    print("完了．出力ファイル一覧:")
    for f in sorted(os.listdir(out_dir)):
        size_kb = os.path.getsize(os.path.join(out_dir, f)) / 1024
        print(f"  {f:35s} ({size_kb:6.1f} KB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
