#!/usr/bin/env python3
"""
visualize_sweep.py — Hegselmann & Krause (2002) BC モデル スイープ結果の可視化．

ε 走査における (上段) 生存意見数の平均±バンド (論文 Fig. 3 / 12a 風) と
(下段) 最終平均意見の平均 (論文 Fig. 12c 風: 対称なので 0.5 付近に張り付くことを
確認できる) を上下 2 段で表示する．

1 行 1 試行の表は，スイープ親 run の子 (`subcommand=sweep-point`) の
`events.jsonl` から組み直す．散らばり (±1σ) を描くには条件ごとの平均ではなく
個々の試行が要るので，子 run の run スコープ集約ではなく終端イベントを読む．
runvault 以前の `sweep_summary.csv` もそのまま読める．

Usage:
    uv run hegselmann-bc-tools visualize-sweep
    uv run hegselmann-bc-tools visualize-sweep --sweep-dir "$(runvault path --experiment hegselmann-bc --latest --subcommand sweep)"
"""

from __future__ import annotations

import argparse
import os

import matplotlib.pyplot as plt
import pandas as pd
from runvault.read import figures_dir, runvault_path, sweep_events_table

# --------------------------------------------------------------------------- #
# 表示設定
# --------------------------------------------------------------------------- #
try:
    plt.rcParams["font.family"] = "Hiragino Sans"
except Exception:  # pragma: no cover
    pass

COLOR_BG = "#FAFAF8"
COLOR_LINE = "#534AB7"
COLOR_BAND = "#534AB7"
COLOR_MEAN_LINE = "#0F6E56"
COLOR_REF = "#888888"


# --------------------------------------------------------------------------- #
# データ
# --------------------------------------------------------------------------- #

def load_summary(sweep_dir: str) -> pd.DataFrame:
    """1 行 1 試行の表 (`eps`, `run_id`, 最終メトリクス) を返す．

    runvault はこの表をディスクに持たないので，スイープ親の子 run の終端イベント
    から組み直す．対称スイープなので条件を表す列は `eps_l` 1 本で足り，本モデルの
    表記に合わせて `eps` に，`unit_id` (`trial-<i>`) は `run_id` に直す．

    runvault 以前のスイープには `sweep_summary.csv` が残っているので，あればそちら
    を読む．そちらが正本だった時期の結果を読めなくする理由はない．
    """
    legacy = os.path.join(sweep_dir, "sweep_summary.csv")
    if os.path.exists(legacy):
        return pd.read_csv(legacy)

    df = sweep_events_table(sweep_dir, ["eps_l"], kind="terminal")
    df = df.rename(columns={"eps_l": "eps"})
    df["run_id"] = df["unit_id"].str.removeprefix("trial-").astype(int)
    df["converged"] = ~df["censored"]
    df["final_iteration"] = df["t"]
    return df


def aggregate(df: pd.DataFrame) -> pd.DataFrame:
    agg = (
        df.groupby("eps")
        .agg(
            n_mean=("n_surviving", "mean"),
            n_std=("n_surviving", "std"),
            mean_mean=("mean", "mean"),
            mean_std=("mean", "std"),
        )
        .reset_index()
    )
    agg["n_std"] = agg["n_std"].fillna(0.0)
    agg["mean_std"] = agg["mean_std"].fillna(0.0)
    return agg.sort_values("eps")


# --------------------------------------------------------------------------- #
# 可視化
# --------------------------------------------------------------------------- #

def save_sweep_overview(agg: pd.DataFrame, out_path: str) -> None:
    fig, axes = plt.subplots(2, 1, figsize=(9, 8), facecolor=COLOR_BG, sharex=True)
    fig.suptitle(
        "Hegselmann–Krause (2002) BC 力学 — ε 走査 (相図; 論文 Fig. 3 / 12 風)",
        fontsize=13,
    )

    # 上段: 生存意見数 vs ε (Fig. 3 / 12a)
    ax = axes[0]
    ax.set_facecolor(COLOR_BG)
    ax.plot(agg["eps"], agg["n_mean"], color=COLOR_LINE, lw=2.0, marker="o", markersize=4)
    ax.fill_between(
        agg["eps"],
        agg["n_mean"] - agg["n_std"],
        agg["n_mean"] + agg["n_std"],
        color=COLOR_BAND,
        alpha=0.15,
    )
    ax.axhline(1.0, color=COLOR_REF, lw=0.8, linestyle="--", label="合意境界 (1 クラスタ)")
    ax.set_ylabel("生存意見数 (試行平均)")
    ax.set_title("生存意見数 vs 信頼幅 ε (上段)")
    ax.set_yscale("log")
    ax.grid(True, alpha=0.3, which="both")
    ax.legend(fontsize=9)

    # 下段: 最終平均意見 vs ε (Fig. 12c) — 対称 BC では 0.5 近傍に張り付く．
    ax = axes[1]
    ax.set_facecolor(COLOR_BG)
    ax.plot(agg["eps"], agg["mean_mean"], color=COLOR_MEAN_LINE, lw=2.0, marker="s", markersize=4)
    ax.fill_between(
        agg["eps"],
        agg["mean_mean"] - agg["mean_std"],
        agg["mean_mean"] + agg["mean_std"],
        color=COLOR_MEAN_LINE,
        alpha=0.15,
    )
    ax.axhline(0.5, color=COLOR_REF, lw=0.8, linestyle="--", label="対称基準 x̄ = 0.5")
    ax.set_xlabel("信頼幅 ε")
    ax.set_ylabel("最終平均意見 (試行平均)")
    ax.set_title("最終平均意見 vs 信頼幅 ε (下段; 対称 BC は 0.5 付近)")
    ax.set_ylim(0.0, 1.0)
    ax.grid(True, alpha=0.3)
    ax.legend(fontsize=9)

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


# --------------------------------------------------------------------------- #
# メイン
# --------------------------------------------------------------------------- #

def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="hegselmann-bc-tools visualize-sweep",
        description="Hegselmann & Krause (2002) BC モデル スイープ結果 可視化",
    )
    p.add_argument(
        "--sweep-dir", "--sweep_dir", "--results-dir", "--results_dir",
        default=None,
        help=(
            "スイープ親 run のディレクトリ．未指定時は runvault に最新のスイープを "
            "聞く (--experiment hegselmann-bc --subcommand sweep)．"
        ),
    )
    p.add_argument(
        "--results-root", "--results_root", default="results",
        help="--sweep-dir 未指定時に runvault が探す results ルート (default: results)",
    )
    p.add_argument(
        "--output-dir", "--output_dir", default=None,
        help="図の保存先ディレクトリ (default: results/hegselmann-bc/figures/{run_slug})",
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)

    sweep_dir = args.sweep_dir
    if sweep_dir is None:
        sweep_dir = runvault_path(
            "hegselmann-bc", args.results_root, subcommand="sweep"
        )

    out_dir = args.output_dir if args.output_dir else figures_dir(sweep_dir)
    os.makedirs(out_dir, exist_ok=True)

    print("=== Hegselmann–Krause (2002) BC 力学 スイープ可視化 ===")
    print(f"スイープ: {sweep_dir}")
    print(f"出力先:   {out_dir}")
    print("-------------------------------------------------")

    print("[1/2] 子 run の終端イベントから 1 行 1 試行の表を組み立て中 ...")
    df = load_summary(sweep_dir)
    agg = aggregate(df)
    print(f"      ε 値 {df['eps'].nunique()} × 試行 {df['run_id'].nunique()}")

    print("[2/2] 相図を保存中 ...")
    save_sweep_overview(agg, os.path.join(out_dir, "visualize_sweep.png"))

    print("-------------------------------------------------")
    # 合意ブリンク (生存意見数の試行平均が初めて 1 になる最小 ε)
    consensus_eps = agg[agg["n_mean"].round() <= 1.0]
    if not consensus_eps.empty:
        brink = float(consensus_eps["eps"].min())
        print(f"合意ブリンク ε* (生存意見数 ≈ 1 になる最小 ε): {brink:.4f}")
    else:
        print("合意ブリンク ε* : 未到達 (走査範囲では合意に達しなかった)")

    print("-------------------------------------------------")
    print("完了．出力ファイル一覧:")
    for f in sorted(os.listdir(out_dir)):
        size_kb = os.path.getsize(os.path.join(out_dir, f)) / 1024
        print(f"  {f:35s} ({size_kb:6.1f} KB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
