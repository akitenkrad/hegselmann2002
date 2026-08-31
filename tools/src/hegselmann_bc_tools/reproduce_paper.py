"""reproduce_paper.py — Hegselmann & Krause (2002) 論文 Figure 一括再現スクリプト．

論文 (JASSS 5(3), 2) の主要 Figure (2 / 3 / 7 / 8 / 11 / 12) を，
Rust バイナリ (`cargo run --release -- run / sweep ...`) の単発呼び出しを
連結して一括再現する．各 Figure ごとに対応する CSV を読み込み，PNG を
`results/reproduce_<timestamp>/figures/` に集約する．

中間データ (`artifacts/opinions.csv` / `metrics.csv` / `events.jsonl`) は
runvault の run ディレクトリ (`results/hegselmann-bc/<run_slug>/`) に残り，
`reproduce_summary.json` にそのパスが記録される．どの run が今の呼び出しの
出力かは runvault に聞くので，ディレクトリ名や mtime から推測しない．

再現対象:

    fig02 : run    n=625 ε=0.01 uniform max_iter=50 seed=42   → ≈38 クラスタ
    fig07 : run    n=100 ε=0.05 regular max_iter=50 seed=1    → 8 splits
    fig08 : run    n=100 ε=0.25 regular max_iter=30 seed=1    → 合意
    fig03 : sweep  n=625 ε∈[0.01,0.40] step=0.01 runs=50 seed=42
    fig12 : sweep  (fig03 のデータを流用; 平均 + 分散の 2 段プロット)
    fig11 : run×4 非対称 (ε_l,ε_r)∈{(.2,.2)/(.15,.25)/(.10,.30)/(.05,.35)},
            n=625 uniform max_iter=100 seed=42  → 2×2 パネル

Usage:
    uv run hegselmann-bc-tools reproduce
    uv run hegselmann-bc-tools reproduce --quick           # 軽量版 (動作確認用)
    uv run hegselmann-bc-tools reproduce --specs fig02,fig03
    uv run hegselmann-bc-tools reproduce --output-dir results --workspace-root .
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Callable

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
from runvault.read import artifacts_dir, config_parameters, runvault_path

from hegselmann_bc_tools.visualize import (
    COLOR_BG,
    COLOR_CLUSTER,
    COLOR_TRAJ,
    load_metrics,
    load_opinions,
    save_opinion_trajectory,
    to_wide,
)
from hegselmann_bc_tools.visualize_sweep import (
    COLOR_BAND,
    COLOR_LINE,
    COLOR_MEAN_LINE,
    COLOR_REF,
    aggregate,
    load_summary,
    save_sweep_overview,
)

# --------------------------------------------------------------------------- #
# 共通定数 / プロジェクトルート解決
# --------------------------------------------------------------------------- #

# このモジュールは tools/src/hegselmann_bc_tools/reproduce_paper.py にある．
# parents[3] が workspace ルート (= cargo workspace ルート)．
# 環境変数 HEGSELMANN_BC_PROJECT_ROOT で上書き可能．
_env_root = os.environ.get("HEGSELMANN_BC_PROJECT_ROOT")
if _env_root:
    PROJECT_ROOT = Path(_env_root).resolve()
else:
    PROJECT_ROOT = Path(__file__).resolve().parents[3]


# --------------------------------------------------------------------------- #
# Figure 仕様データ構造
# --------------------------------------------------------------------------- #


@dataclass
class FigureSpec:
    """1 つの Figure を生成するための単発実行仕様．

    Attributes:
        id: フィギュア ID (ファイル名・summary キーに使う)．
        subcommand: cargo のサブコマンド (`run` / `sweep`)．`fig11` のような
            複数実行を束ねるものは `run` を入れて `cli_args_list` を使う．
        cli_args: 1 回限りの cargo 呼び出しの引数列．`run`/`sweep` のいずれか．
        cli_args_list: 複数回呼び出す場合の引数列のリスト (fig11 用)．
        description: 1 行説明．
        render: 描画関数 (各 spec ごとに渡す)．
        output_basename: PNG ファイル名 (拡張子なし)．
        derived_from: 別 spec の出力 CSV を流用する場合のソース ID．
            例: fig12 は fig03 のスイープ CSV を流用する．
    """

    id: str
    subcommand: str
    description: str
    output_basename: str
    cli_args: list[str] | None = None
    cli_args_list: list[list[str]] | None = None
    derived_from: str | None = None
    # render(spec, run_dirs, figures_dir) -> figure file path
    render: Callable[["FigureSpec", list[Path], Path], Path] | None = None


# --------------------------------------------------------------------------- #
# cargo 呼び出しヘルパ
# --------------------------------------------------------------------------- #


def ensure_build() -> None:
    """`cargo build --release` を 1 度だけ実行する (失敗時は例外)．"""
    print("=== cargo build --release ===")
    subprocess.run(
        ["cargo", "build", "--release"],
        cwd=PROJECT_ROOT,
        check=True,
    )


def run_cargo(args: list[str], output_dir: Path) -> Path:
    """`cargo run --release -- ...` を呼び出し，その run ディレクトリを返す．

    どこに落ちたかは runvault に聞く (`runvault path --latest`)．出力先は
    `<output_dir>/hegselmann-bc/<run_slug>/` で，run_slug には条件と環境の
    ハッシュが入るため，こちら側で名前を組み立てることも，mtime で当てにいく
    こともできない (できたとしてもすべきでない — 名前の決め方は runvault の
    持ちものである)．

    `run` は `--standalone` で絞る．スイープの子は別サブコマンド
    (`sweep-point`) なので混ざらないが，`run` を手で連続実行したときに
    «最後に走った run» を返す契約であることを明示しておく．

    Args:
        args: cargo の `--` 以降に渡す引数列．先頭は `run` / `sweep` 等．
        output_dir: `--output-dir` に渡すディレクトリ (workspace 相対 or 絶対)．

    Returns:
        この呼び出しが作った run ディレクトリ (絶対パス)．
    """
    output_dir.mkdir(parents=True, exist_ok=True)

    subcommand = args[0] if args else "run"

    cmd = ["cargo", "run", "--release", "--quiet", "--"] + args + [
        "--output-dir", str(output_dir),
    ]
    subprocess.run(cmd, cwd=PROJECT_ROOT, check=True, stdout=subprocess.DEVNULL)

    return Path(
        runvault_path(
            "hegselmann-bc",
            str(output_dir),
            subcommand=subcommand,
            standalone=(subcommand == "run"),
        )
    )


# --------------------------------------------------------------------------- #
# 描画関数
# --------------------------------------------------------------------------- #


def _render_run_trajectory(
    spec: FigureSpec, run_dirs: list[Path], figures_dir: Path,
) -> Path:
    """run 1 回分の opinion_trajectory を生成し，figures_dir に保存する．"""
    assert len(run_dirs) == 1, f"run trajectory には run_dirs 1 個が必要: {len(run_dirs)}"
    rd = run_dirs[0]
    df_op = load_opinions(os.path.join(artifacts_dir(rd), "opinions.csv"))
    ts, mat = to_wide(df_op)
    df_m = load_metrics(str(rd / "metrics.csv"))
    out_path = figures_dir / f"{spec.output_basename}.png"
    save_opinion_trajectory(
        ts, mat, df_m, str(out_path),
        subtitle=f"{spec.id}: {spec.description}",
    )
    return out_path


def _render_sweep_overview(
    spec: FigureSpec, run_dirs: list[Path], figures_dir: Path,
) -> Path:
    """sweep の Fig. 3 風 (生存意見数 + 平均) 2 段プロットを保存する．"""
    assert len(run_dirs) == 1, f"sweep overview には run_dirs 1 個が必要: {len(run_dirs)}"
    sweep_dir = run_dirs[0]
    df = load_summary(str(sweep_dir))
    agg = aggregate(df)
    out_path = figures_dir / f"{spec.output_basename}.png"
    save_sweep_overview(agg, str(out_path))
    return out_path


def _save_sweep_mean_variance(agg: pd.DataFrame, out_path: Path) -> None:
    """論文 Fig. 12 風: ε に対する平均意見 (上段) と分散 (下段) を 2 段表示．

    aggregate() は (eps, n_mean/std, mean_mean/std) しか返さないので，分散統計は
    元の long-format から別途集計する必要がある．本関数は呼び出し側で
    `mean_variance_agg` を構築済みである前提とする．
    """
    fig, axes = plt.subplots(2, 1, figsize=(9, 7.5), facecolor=COLOR_BG, sharex=True)
    fig.suptitle(
        "Hegselmann–Krause (2002) BC 力学 — 論文 Fig. 12 風 (最終平均 + 分散)",
        fontsize=13,
    )

    # 上段: 最終平均意見
    ax = axes[0]
    ax.set_facecolor(COLOR_BG)
    ax.plot(agg["eps"], agg["mean_mean"], color=COLOR_MEAN_LINE, lw=2.0,
            marker="s", markersize=4)
    ax.fill_between(
        agg["eps"],
        agg["mean_mean"] - agg["mean_std"],
        agg["mean_mean"] + agg["mean_std"],
        color=COLOR_MEAN_LINE, alpha=0.15,
    )
    ax.axhline(0.5, color=COLOR_REF, lw=0.8, linestyle="--",
               label="対称基準 x̄ = 0.5")
    ax.set_ylabel("最終平均意見 (試行平均)")
    ax.set_title("最終平均意見 vs 信頼幅 ε (上段)")
    ax.set_ylim(0.0, 1.0)
    ax.grid(True, alpha=0.3)
    ax.legend(fontsize=9)

    # 下段: 最終分散
    ax = axes[1]
    ax.set_facecolor(COLOR_BG)
    ax.plot(agg["eps"], agg["var_mean"], color=COLOR_LINE, lw=2.0,
            marker="o", markersize=4)
    ax.fill_between(
        agg["eps"],
        agg["var_mean"] - agg["var_std"],
        agg["var_mean"] + agg["var_std"],
        color=COLOR_BAND, alpha=0.15,
    )
    ax.set_xlabel("信頼幅 ε")
    ax.set_ylabel("最終意見分散 (試行平均)")
    ax.set_title("最終意見分散 vs 信頼幅 ε (下段; 0 へ向かう ⇒ 合意)")
    ax.grid(True, alpha=0.3)

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


def _render_sweep_mean_variance(
    spec: FigureSpec, run_dirs: list[Path], figures_dir: Path,
) -> Path:
    """論文 Fig. 12 風: 最終平均 + 最終分散の 2 段プロットを生成する．"""
    assert len(run_dirs) == 1, f"sweep mean_variance には run_dirs 1 個が必要: {len(run_dirs)}"
    sweep_dir = run_dirs[0]
    df = load_summary(str(sweep_dir))
    # 平均・分散の試行平均と標準偏差を再集計する
    agg = (
        df.groupby("eps")
        .agg(
            mean_mean=("mean", "mean"),
            mean_std=("mean", "std"),
            var_mean=("variance", "mean"),
            var_std=("variance", "std"),
        )
        .reset_index()
    )
    agg["mean_std"] = agg["mean_std"].fillna(0.0)
    agg["var_std"] = agg["var_std"].fillna(0.0)
    agg = agg.sort_values("eps")
    out_path = figures_dir / f"{spec.output_basename}.png"
    _save_sweep_mean_variance(agg, out_path)
    return out_path


def _render_fig11_panel(
    spec: FigureSpec, run_dirs: list[Path], figures_dir: Path,
) -> Path:
    """論文 Fig. 11 風: 非対称 BC の 2×2 パネル (4 つの (ε_l, ε_r) 設定)．

    各サブプロットに opinion trajectory を半透明線で描き，最終平均値を
    水平太線で重ね描きする．タイトルに `ε_l=..., ε_r=...` と最終平均を注釈．
    """
    assert len(run_dirs) == 4, (
        f"fig11 パネルには run_dirs 4 個が必要: {len(run_dirs)}"
    )
    fig, axes = plt.subplots(2, 2, figsize=(12, 9), facecolor=COLOR_BG, sharex=True, sharey=True)
    fig.suptitle(
        "Hegselmann–Krause (2002) BC 力学 — 論文 Fig. 11 風 (非対称 BC; 2×2)",
        fontsize=14,
    )

    for ax, rd in zip(axes.flat, run_dirs):
        ax.set_facecolor(COLOR_BG)

        # config.json は封筒なので，条件は parameters の下から取り出す
        cfg = config_parameters(rd) or {}
        eps_l = cfg.get("eps_l", float("nan"))
        eps_r = cfg.get("eps_r", float("nan"))

        df_op = load_opinions(os.path.join(artifacts_dir(rd), "opinions.csv"))
        ts, mat = to_wide(df_op)
        n_agents = mat.shape[1]
        alpha = max(0.05, min(0.6, 30.0 / max(n_agents, 1)))
        lw = 0.5 if n_agents > 200 else 0.8
        for i in range(n_agents):
            ax.plot(ts, mat[:, i], color=COLOR_TRAJ, alpha=alpha, lw=lw)

        # 最終平均
        final_mean = float(np.mean(mat[-1]))
        ax.axhline(final_mean, color=COLOR_CLUSTER, lw=1.6, alpha=0.85,
                   linestyle="--")
        # 終端強調マーカ
        ax.scatter([ts[-1]] * n_agents, mat[-1], color=COLOR_CLUSTER,
                   s=4, alpha=0.4, zorder=5)

        ax.set_title(
            f"ε_l = {eps_l:.2f}, ε_r = {eps_r:.2f}   "
            f"最終平均 x̄ = {final_mean:.3f}",
            fontsize=11,
        )
        ax.set_ylim(-0.02, 1.02)
        ax.set_xlim(ts.min(), ts.max())
        ax.grid(True, alpha=0.3)

    for ax in axes[-1, :]:
        ax.set_xlabel("時刻 t")
    for ax in axes[:, 0]:
        ax.set_ylabel("意見 x ∈ [0, 1]")

    fig.tight_layout()
    out_path = figures_dir / f"{spec.output_basename}.png"
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")
    return out_path


# --------------------------------------------------------------------------- #
# Figure 仕様カタログ
# --------------------------------------------------------------------------- #


def _build_specs(*, quick: bool) -> list[FigureSpec]:
    """論文の主要 Figure 仕様カタログを構築する (quick モードで一部軽量化)．"""

    # Fig. 2: n=625 (quick=125), ε=0.01, uniform, max_iter=50, seed=42
    n_fig02 = 125 if quick else 625
    fig02 = FigureSpec(
        id="fig02",
        subcommand="run",
        description=f"n={n_fig02}, ε=0.01, uniform — 約 38 クラスタ (fragmentation)",
        output_basename=f"fig02_n{n_fig02}_eps0.01_uniform",
        cli_args=[
            "run",
            "--n", str(n_fig02),
            "--eps", "0.01",
            "--start", "uniform",
            "--max-iterations", "50",
            "--seed", "42",
        ],
        render=_render_run_trajectory,
    )

    # Fig. 7: n=100, ε=0.05, regular, max_iter=50, seed=1 → 8 splits
    fig07 = FigureSpec(
        id="fig07",
        subcommand="run",
        description="n=100, ε=0.05, regular — 8 splits (polarization)",
        output_basename="fig07_n100_eps0.05_regular",
        cli_args=[
            "run",
            "--n", "100",
            "--eps", "0.05",
            "--start", "regular",
            "--max-iterations", "50",
            "--seed", "1",
        ],
        render=_render_run_trajectory,
    )

    # Fig. 8: n=100, ε=0.25, regular, max_iter=30, seed=1 → consensus
    fig08 = FigureSpec(
        id="fig08",
        subcommand="run",
        description="n=100, ε=0.25, regular — consensus",
        output_basename="fig08_n100_eps0.25_regular",
        cli_args=[
            "run",
            "--n", "100",
            "--eps", "0.25",
            "--start", "regular",
            "--max-iterations", "30",
            "--seed", "1",
        ],
        render=_render_run_trajectory,
    )

    # Fig. 3 + Fig. 12: 同じ sweep データを 2 つの図で流用する
    # quick モードでは runs=5, n=125 に削減
    runs_sweep = 5 if quick else 50
    n_sweep = 125 if quick else 625
    sweep_args = [
        "sweep",
        "--n", str(n_sweep),
        "--eps-min", "0.01",
        "--eps-max", "0.40",
        "--eps-step", "0.01",
        "--runs", str(runs_sweep),
        "--start", "uniform",
        "--seed", "42",
    ]
    fig03 = FigureSpec(
        id="fig03",
        subcommand="sweep",
        description=(
            f"n={n_sweep}, ε∈[0.01,0.40] step=0.01, runs={runs_sweep} — "
            "生存意見数の急減 (3 相転移)"
        ),
        output_basename="fig03_sweep_n_surviving",
        cli_args=sweep_args,
        render=_render_sweep_overview,
    )
    fig12 = FigureSpec(
        id="fig12",
        subcommand="sweep",
        description=(
            f"fig03 の sweep を流用 — 最終平均 + 分散 (n={n_sweep}, "
            f"runs={runs_sweep})"
        ),
        output_basename="fig12_sweep_mean_variance",
        derived_from="fig03",
        render=_render_sweep_mean_variance,
    )

    # Fig. 11: 4 つの非対称設定．n=625 固定 (quick でも縮小しない;
    # 4 run × max_iter 100 で数秒〜十数秒程度)
    fig11_pairs = [
        (0.20, 0.20),
        (0.15, 0.25),
        (0.10, 0.30),
        (0.05, 0.35),
    ]
    fig11_args_list = [
        [
            "run",
            "--n", "625",
            "--eps-l", f"{el}",
            "--eps-r", f"{er}",
            "--start", "uniform",
            "--max-iterations", "100",
            "--seed", "42",
        ]
        for el, er in fig11_pairs
    ]
    fig11 = FigureSpec(
        id="fig11",
        subcommand="run",
        description=(
            "n=625, 4 つの非対称 (ε_l, ε_r) — 右側 ε_r が広いほど最終平均が右へシフト"
        ),
        output_basename="fig11_asymmetric_panel",
        cli_args_list=fig11_args_list,
        render=_render_fig11_panel,
    )

    return [fig02, fig07, fig08, fig03, fig12, fig11]


# --------------------------------------------------------------------------- #
# 実行ドライバ
# --------------------------------------------------------------------------- #


def _execute_spec(
    spec: FigureSpec,
    cargo_output_dir: Path,
    spec_results: dict[str, dict],
) -> tuple[list[Path], list[str]]:
    """1 つの spec に対して cargo を必要回数呼び出し，run ディレクトリを返す．

    Returns:
        (run_dirs, cargo_invocations) — run_dirs はこの spec が生成した結果
        ディレクトリ群．cargo_invocations は人間可読な引数列スナップ．
    """
    # derived_from が指定されていれば，依存元の結果を流用する
    if spec.derived_from is not None:
        src = spec_results.get(spec.derived_from)
        if src is None or not src.get("run_dirs"):
            raise RuntimeError(
                f"{spec.id}: 依存元 {spec.derived_from} が未実行か失敗しています"
            )
        return [Path(p) for p in src["run_dirs"]], []

    invocations: list[str] = []
    run_dirs: list[Path] = []

    if spec.cli_args_list is not None:
        for args in spec.cli_args_list:
            invocations.append("cargo run --release -- " + " ".join(args))
            rd = run_cargo(args, cargo_output_dir)
            run_dirs.append(rd)
    elif spec.cli_args is not None:
        invocations.append("cargo run --release -- " + " ".join(spec.cli_args))
        rd = run_cargo(spec.cli_args, cargo_output_dir)
        run_dirs.append(rd)
    else:
        raise ValueError(f"{spec.id}: cli_args / cli_args_list / derived_from のどれかが必要")

    return run_dirs, invocations


def reproduce(
    spec_ids: list[str] | None,
    output_root: Path,
    cargo_output_dir: Path,
    quick: bool,
    skip_build: bool,
) -> dict:
    """指定 spec を順に実行し，まとめた結果サマリを返す．"""
    all_specs = _build_specs(quick=quick)
    if spec_ids:
        wanted = set(spec_ids)
        specs = [s for s in all_specs if s.id in wanted]
        unknown = wanted - {s.id for s in all_specs}
        if unknown:
            raise ValueError(
                f"未知の spec ID: {sorted(unknown)}．"
                f"利用可能: {[s.id for s in all_specs]}"
            )
    else:
        specs = all_specs

    # derived_from の依存元が specs に含まれていなければ自動で追加する
    selected_ids = {s.id for s in specs}
    for s in list(specs):
        if s.derived_from and s.derived_from not in selected_ids:
            src_spec = next((x for x in all_specs if x.id == s.derived_from), None)
            if src_spec is None:
                raise ValueError(
                    f"{s.id}: 依存元 spec {s.derived_from} がカタログにありません"
                )
            specs.insert(0, src_spec)
            selected_ids.add(src_spec.id)

    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    base_dir = output_root / f"reproduce_{timestamp}"
    figures_dir = base_dir / "figures"
    base_dir.mkdir(parents=True, exist_ok=True)
    figures_dir.mkdir(parents=True, exist_ok=True)

    print("=== Hegselmann & Krause (2002) 論文 Figure 一括再現 ===")
    print(f"    出力ルート     : {base_dir}")
    print(f"    cargo 出力先   : {cargo_output_dir}")
    print(f"    Figure 出力先  : {figures_dir}")
    print(f"    quick モード   : {quick}")
    print(f"    対象 spec      : {[s.id for s in specs]}")
    print("-------------------------------------------")

    if not skip_build:
        ensure_build()

    spec_results: dict[str, dict] = {}
    for spec in specs:
        print(f"--- {spec.id}: {spec.description} ---")
        t0 = time.monotonic()
        try:
            run_dirs, invocations = _execute_spec(spec, cargo_output_dir, spec_results)
            cargo_elapsed = time.monotonic() - t0

            t1 = time.monotonic()
            figure_path = spec.render(spec, run_dirs, figures_dir) if spec.render else None
            render_elapsed = time.monotonic() - t1

            elapsed = time.monotonic() - t0
            spec_results[spec.id] = {
                "id": spec.id,
                "description": spec.description,
                "subcommand": spec.subcommand,
                "cli_args": spec.cli_args,
                "cli_args_list": spec.cli_args_list,
                "derived_from": spec.derived_from,
                "cargo_invocations": invocations,
                "run_dirs": [str(p) for p in run_dirs],
                "figure_path": str(figure_path) if figure_path else None,
                "status": "ok",
                "cargo_seconds": round(cargo_elapsed, 3),
                "render_seconds": round(render_elapsed, 3),
                "total_seconds": round(elapsed, 3),
            }
            print(
                f"  ✓ {spec.id} done in {elapsed:.2f}s "
                f"(cargo {cargo_elapsed:.2f}s + render {render_elapsed:.2f}s)"
            )
        except Exception as e:  # noqa: BLE001
            elapsed = time.monotonic() - t0
            spec_results[spec.id] = {
                "id": spec.id,
                "description": spec.description,
                "subcommand": spec.subcommand,
                "cli_args": spec.cli_args,
                "cli_args_list": spec.cli_args_list,
                "derived_from": spec.derived_from,
                "status": "error",
                "error": repr(e),
                "total_seconds": round(elapsed, 3),
            }
            print(f"  ✗ {spec.id} failed: {e}", file=sys.stderr)

    # サマリ JSON を保存
    summary = {
        "timestamp": timestamp,
        "quick": quick,
        "project_root": str(PROJECT_ROOT),
        "base_dir": str(base_dir),
        "figures_dir": str(figures_dir),
        "cargo_output_dir": str(cargo_output_dir),
        "specs": list(spec_results.values()),
    }
    summary_path = base_dir / "reproduce_summary.json"
    with summary_path.open("w") as f:
        json.dump(summary, f, indent=2, ensure_ascii=False)

    print("-------------------------------------------")
    n_ok = sum(1 for r in spec_results.values() if r.get("status") == "ok")
    n_err = sum(1 for r in spec_results.values() if r.get("status") == "error")
    print(f"完了: ok={n_ok}, error={n_err}")
    print(f"サマリ → {summary_path}")
    print(f"図一覧 → {figures_dir}")
    for f in sorted(figures_dir.iterdir()):
        if f.is_file():
            size_kb = f.stat().st_size / 1024
            print(f"    {f.name:45s} ({size_kb:6.1f} KB)")

    return summary


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="hegselmann-bc-tools reproduce",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    p.add_argument(
        "--specs", default=None,
        help=(
            "カンマ区切りで実行する spec ID (例: fig02,fig03)．"
            "未指定時は全 spec を実行する．利用可能: fig02,fig03,fig07,fig08,fig11,fig12"
        ),
    )
    p.add_argument(
        "--output-dir", "--output_dir", default="results",
        help=(
            "結果出力ルート (workspace ルートからの相対パス)．"
            "PNG とサマリはここの reproduce_<ts>/ 配下に保存される (default: results)"
        ),
    )
    p.add_argument(
        "--cargo-output-dir", "--cargo_output_dir", default=None,
        help=(
            "cargo の --output-dir に渡すパス．未指定時は --output-dir と同じ "
            "(つまり Rust 出力は results/<inner_ts>/ に置かれる)．"
        ),
    )
    p.add_argument(
        "--workspace-root", "--workspace_root", default=None,
        help=(
            "workspace ルート (絶対パス)．未指定時は本モジュールの位置から "
            "推定する (環境変数 HEGSELMANN_BC_PROJECT_ROOT でも上書き可)．"
        ),
    )
    p.add_argument(
        "--quick", action="store_true",
        help=(
            "簡略化モード: fig02/fig03/fig12 を縮小実行 (n=125, runs=5)．"
            "動作確認用．論文値の検証には使わない．"
        ),
    )
    p.add_argument(
        "--skip-build", action="store_true",
        help="cargo build --release をスキップ (事前にビルド済みのとき)．",
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)

    # workspace ルートの上書き
    global PROJECT_ROOT
    if args.workspace_root:
        PROJECT_ROOT = Path(args.workspace_root).resolve()

    if shutil.which("cargo") is None:
        print(
            "エラー: cargo コマンドが見つかりません．Rust toolchain をインストールしてください．",
            file=sys.stderr,
        )
        return 2

    # 出力ディレクトリ解決 (相対は workspace 相対)
    output_root = Path(args.output_dir)
    if not output_root.is_absolute():
        output_root = PROJECT_ROOT / output_root

    if args.cargo_output_dir is not None:
        cargo_output_dir = Path(args.cargo_output_dir)
    else:
        cargo_output_dir = output_root
    if not cargo_output_dir.is_absolute():
        cargo_output_dir = PROJECT_ROOT / cargo_output_dir

    spec_ids = None
    if args.specs:
        spec_ids = [s.strip() for s in args.specs.split(",") if s.strip()]

    try:
        summary = reproduce(
            spec_ids=spec_ids,
            output_root=output_root,
            cargo_output_dir=cargo_output_dir,
            quick=args.quick,
            skip_build=args.skip_build,
        )
    except Exception as e:  # noqa: BLE001
        print(f"エラー: 再現実行に失敗しました: {e}", file=sys.stderr)
        return 1

    # 1 つでも失敗があれば非 0 を返す
    n_err = sum(1 for r in summary["specs"] if r.get("status") == "error")
    return 0 if n_err == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
