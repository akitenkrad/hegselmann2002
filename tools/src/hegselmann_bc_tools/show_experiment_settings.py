"""hegselmann-bc-tools show-experiment-settings — 実行結果の設定表示．

results/{timestamp}/config.json (run) または
results/{timestamp}_sweep/sweep_config.json (sweep) を読み，実行時に使われた
全パラメータを整形表示する．`results/latest` も解決される．

Usage:
    hegselmann-bc-tools show-experiment-settings
    hegselmann-bc-tools show-experiment-settings --results-dir results/20260528_153000
    hegselmann-bc-tools show-experiment-settings --results-dir results/latest --json
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path


def resolve_results_dir(path_like: str) -> Path:
    """`results/latest` シンボリックリンクを実体に解決する．"""
    p = Path(path_like)
    if p.is_symlink():
        return Path(os.path.realpath(p))
    return p


def _find_config_file(results_dir: Path) -> tuple[Path, str]:
    run_cfg = results_dir / "config.json"
    sweep_cfg = results_dir / "sweep_config.json"
    if run_cfg.exists():
        return run_cfg, "run"
    if sweep_cfg.exists():
        return sweep_cfg, "sweep"
    raise FileNotFoundError(
        f"設定ファイルが見つかりません: {results_dir}\n"
        f"  期待されるファイル: config.json (run) または sweep_config.json (sweep)"
    )


def render_run_config(cfg: dict, source: Path) -> str:
    lines: list[str] = []
    lines.append("=" * 70)
    lines.append("実行設定 (run)")
    lines.append("=" * 70)
    lines.append(f"設定ファイル: {source}")
    lines.append("-" * 70)
    lines.append(f"エージェント数 n : {cfg.get('n', '-')}")
    lines.append(f"信頼幅 ε_l       : {cfg.get('eps_l', '-')}")
    lines.append(f"信頼幅 ε_r       : {cfg.get('eps_r', '-')}")
    lines.append(f"対称 (eps_l==eps_r) : {cfg.get('symmetric', '-')}")
    lines.append(f"初期分布         : {cfg.get('start_profile', '-')}")
    lines.append(f"最大反復         : {cfg.get('max_iterations', '-')}")
    lines.append(f"収束許容誤差 tol : {cfg.get('tol', '-')}")
    lines.append(f"シード           : {cfg.get('seed', '-')}")
    lines.append(f"出力先           : {cfg.get('output_dir', '-')}")
    lines.append("=" * 70)
    return "\n".join(lines)


def render_sweep_config(cfg: dict, source: Path) -> str:
    lines: list[str] = []
    lines.append("=" * 70)
    lines.append("実行設定 (sweep)")
    lines.append("=" * 70)
    lines.append(f"設定ファイル: {source}")
    lines.append("-" * 70)
    lines.append(
        f"ε 走査           : {cfg.get('eps_min', '-')}:{cfg.get('eps_max', '-')}:{cfg.get('eps_step', '-')}"
    )
    lines.append(f"エージェント数 n : {cfg.get('n', '-')}")
    lines.append(f"試行数 runs      : {cfg.get('runs', '-')}")
    lines.append(f"初期分布         : {cfg.get('start_profile', '-')}")
    lines.append(f"最大反復         : {cfg.get('max_iterations', '-')}")
    lines.append(f"収束許容誤差 tol : {cfg.get('tol', '-')}")
    lines.append(f"シード基点       : {cfg.get('seed', '-')}")
    lines.append("=" * 70)
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="hegselmann-bc-tools show-experiment-settings",
        description="Hegselmann & Krause (2002) BC モデル — 実験設定表示",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--results-dir", "--results_dir",
        default="results/latest",
        help="実行結果ディレクトリ (default: results/latest)",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="表ではなく JSON 形式で出力する．",
    )
    args = parser.parse_args(argv)

    results_dir = resolve_results_dir(args.results_dir)
    if not results_dir.exists():
        print(f"エラー: ディレクトリが存在しません: {results_dir}", file=sys.stderr)
        return 1

    cfg_path, kind = _find_config_file(results_dir)
    with cfg_path.open() as f:
        cfg = json.load(f)

    if args.json:
        payload = {"source": str(cfg_path), "kind": kind, "config": cfg}
        print(json.dumps(payload, indent=2, ensure_ascii=False))
    elif kind == "run":
        print(render_run_config(cfg, cfg_path))
    else:
        print(render_sweep_config(cfg, cfg_path))
    return 0


if __name__ == "__main__":
    sys.exit(main())
