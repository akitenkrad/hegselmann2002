"""hegselmann-bc-tools — Hegselmann & Krause (2002) BC モデル ツール統合 CLI．

Usage:
    hegselmann-bc-tools visualize [...]
    hegselmann-bc-tools visualize-sweep [...]
    hegselmann-bc-tools show-experiment-settings [...]

各サブコマンドに続く引数は，対応するモジュールの argparse がそのまま受け取る．
サブコマンドレベルで `--help` を付けると，そのサブコマンド自身のヘルプが表示される．

姉妹実装 `hegselmann2005` は共有 dispatcher (`socsim_tools.cli.build_dispatcher`)
を使うが，本リポジトリは最小依存にとどめるため stdlib (argparse + importlib) で
同等の dispatcher をローカル実装する．
"""

from __future__ import annotations

import importlib
import sys


SUBCOMMANDS: dict[str, tuple[str, str]] = {
    "visualize": (
        "単一実行結果 (意見軌跡) の可視化",
        "hegselmann_bc_tools.visualize:main",
    ),
    "visualize-sweep": (
        "スイープ結果 (生存意見数の相図) の可視化",
        "hegselmann_bc_tools.visualize_sweep:main",
    ),
    "show-experiment-settings": (
        "実行結果ディレクトリの設定 (config.json / sweep_config.json) の表示",
        "hegselmann_bc_tools.show_experiment_settings:main",
    ),
}


def _resolve(entrypoint: str):
    """`module:func` 形式の entrypoint を実 import して関数を返す．"""
    module_name, func_name = entrypoint.split(":", 1)
    module = importlib.import_module(module_name)
    return getattr(module, func_name)


def main(argv: list[str] | None = None) -> int:
    if argv is None:
        argv = sys.argv[1:]

    # 最小限の手動ディスパッチャ: 最初の非 -h トークンをサブコマンド名として
    # 切り出し，残りを実装の argparse にそのまま渡す．argparse の REMAINDER
    # は親パーサに先に `-` 始まりの未知フラグを取られてしまうので避ける．
    if not argv or argv[0] in ("-h", "--help"):
        names = ", ".join(SUBCOMMANDS.keys())
        print("hegselmann-bc-tools — Hegselmann & Krause (2002) BC モデル 可視化・分析ツール")
        print(f"\nUsage: hegselmann-bc-tools <{names}> [options]")
        print("\nSubcommands:")
        for name, (help_text, _) in SUBCOMMANDS.items():
            print(f"  {name:<28} {help_text}")
        return 0

    command = argv[0]
    rest = argv[1:]
    if command not in SUBCOMMANDS:
        print(f"エラー: 不明なサブコマンド: {command}", file=sys.stderr)
        print(f"  利用可能: {', '.join(SUBCOMMANDS.keys())}", file=sys.stderr)
        return 2

    entrypoint = SUBCOMMANDS[command][1]
    impl = _resolve(entrypoint)
    return impl(rest) or 0


if __name__ == "__main__":
    sys.exit(main())
