#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""FlowType_H のスクリプト（`@FlowType_H.anm2`）だけを生成・検査・配置する。

    python AI/plugins/FlowType_H/build_scripts.py                    # 生成 → 検査 → 配置
    python AI/plugins/FlowType_H/build_scripts.py --no-deploy        # 生成と検査だけ
    python AI/plugins/FlowType_H/build_scripts.py --no-deploy --expect-identical
                                                                     # 配置済みとバイト一致を確かめる

`astra build` は C++ プラグインのビルド（vcpkg が要る）も走らせるので使わない。
astra の `Builder` をスクリプトにだけ使う（`astra.toml` の `[[build.scripts]]`）。
手を付けていない正本から作った `.anm2` が、配置済みの
`Script/FlowType_H/@FlowType_H.anm2` とバイト単位で一致することを確認済み（2026-09-16）。

**`Script/FlowType_H/@FlowType_H.anm2` は生成物。直接編集しない。**
正本は `scripts/effects/*.lua`（`motion.lua` ほか。`#include` される `utilities.lua` など）。

手順
----

1. `build/astra/scripts/effect/@FlowType_H.anm2` へ生成する（`build/` は `.gitignore` 対象）
2. 形を検査する: UTF-8 BOM 無し、先頭が `@モーション`、セクションが astra.toml の sources と同数
3. `AI/tools/check_lua_syntax.py` に通す（マルチセクションをセクション単位で luaJIT に読ませる）
4. 配置済みと比べる（一致 / 相違を出す。`--expect-identical` なら相違で終了コード 1）
5. 配置する（`--no-deploy` で省略）
   - `Script/FlowType_H/@FlowType_H.anm2`
   - `Preset/` へ `presets/*.preset`
   - `Script/FlowType_H/` へ `*.md` と `LICENSE`（astra.toml の release.contents と同じ）
   配置後に読み戻してバイト一致を確かめる

UI 定義（`--track@` `--select@` など）を変えたときは、配置しても
**AviUtl2 の再起動まで反映されない**（「キャッシュを破棄」では足りない）。

終了コード: 0 = 成功 / 1 = 検査で落ちた / 2 = 実行できない（astra が無い等）
"""

import argparse
import io
import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent                   # AI/plugins/FlowType_H
ROOT = HERE.parents[2]                                   # C:\ProgramData\aviutl2
BUILD_DIR = HERE / "build" / "astra"
DEPLOY_SCRIPT = ROOT / "Script" / "FlowType_H" / "@FlowType_H.anm2"
DEPLOY_DOC_DIR = ROOT / "Script" / "FlowType_H"
DEPLOY_PRESET_DIR = ROOT / "Preset"
CHECK_SYNTAX = ROOT / "AI" / "tools" / "check_lua_syntax.py"
DOCS = ("CHANGELOG.md", "README.md", "THIRD_PARTY_LICENSES.md", "LICENSE")

OUT = io.open(1, "w", encoding="utf-8", closefd=False)


def say(text=""):
    OUT.write(text + "\n")
    OUT.flush()


def generate():
    """astra の Builder でスクリプトだけを生成し、生成物のパスを返す。"""
    try:
        from astra._internal.build import Builder
        from astra._internal.config import Build, Config
    except ImportError as e:
        say("[ERROR] astra を読み込めない: %s" % e)
        sys.exit(2)

    cwd = os.getcwd()
    os.chdir(str(HERE))                                  # astra.toml の相対パスはここ基準
    try:
        cfg = Config(HERE / "astra.toml").load(Build)
        builder = Builder(BUILD_DIR, cfg.root, "release")
        produced = []
        expected_sources = 0
        for script in cfg.scripts:
            expected_sources += sum(len(s.files) for s in script.sources)
            for p in builder.build(script):
                produced.append(Path(p))
    finally:
        os.chdir(cwd)

    anm2 = [p for p in produced if p.suffix == ".anm2"]
    if len(anm2) != 1:
        say("[ERROR] 生成物の .anm2 が 1 つではない: %s" % [str(p) for p in produced])
        sys.exit(1)
    return anm2[0], expected_sources


def check_shape(path, expected_sections):
    data = path.read_bytes()
    ok = True
    if data.startswith(b"\xef\xbb\xbf"):
        say("[ERROR] BOM が付いている: %s" % path)
        ok = False
    if not data.startswith("@モーション".encode("utf-8")):
        say("[ERROR] 先頭が @モーション ではない: %r" % data[:20])
        ok = False
    text = data.decode("utf-8")
    sections = [ln for ln in text.splitlines() if ln.startswith("@")]
    if len(sections) != expected_sections:
        say("[ERROR] セクション数 %d（astra.toml の sources は %d）: %s"
            % (len(sections), expected_sections, sections))
        ok = False
    else:
        say("[INFO] セクション %d 個: %s" % (len(sections), " ".join(sections)))
    if "${" in text:
        say("[ERROR] 展開されていない変数が残っている")
        ok = False
    if "--#include" in text or "--#define" in text:
        say("[ERROR] 展開されていない #include / #define が残っている")
        ok = False
    return ok


def check_syntax(path):
    rc = subprocess.call([sys.executable, str(CHECK_SYNTAX), str(path)])
    if rc != 0:
        say("[ERROR] check_lua_syntax.py が終了コード %d" % rc)
    return rc == 0


def compare(generated):
    if not DEPLOY_SCRIPT.is_file():
        say("[INFO] 配置済みの .anm2 が無い: %s" % DEPLOY_SCRIPT)
        return False
    a, b = generated.read_bytes(), DEPLOY_SCRIPT.read_bytes()
    if a == b:
        say("[INFO] 配置済みとバイト一致（%d バイト）" % len(a))
        return True
    la = a.decode("utf-8").splitlines()
    lb = b.decode("utf-8").splitlines()
    import difflib
    changed = sum(1 for ln in difflib.unified_diff(lb, la, lineterm="", n=0)
                  if ln[:1] in "+-" and not ln.startswith(("+++", "---")))
    say("[INFO] 配置済みと相違（生成 %d バイト / 配置済み %d バイト、差分 %d 行）"
        % (len(a), len(b), changed))
    return False


def copy_verified(src, dst):
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(str(src), str(dst))
    if dst.read_bytes() != src.read_bytes():
        say("[ERROR] 配置後の読み戻しが一致しない: %s" % dst)
        return False
    say("[INFO] 配置: %s" % dst.relative_to(ROOT))
    return True


def deploy(generated):
    ok = copy_verified(generated, DEPLOY_SCRIPT)
    presets = sorted((HERE / "presets").glob("*.preset"))
    if not presets:
        say("[ERROR] presets/*.preset が 0 件")
        ok = False
    for p in presets:
        ok = copy_verified(p, DEPLOY_PRESET_DIR / p.name) and ok
    for name in DOCS:
        ok = copy_verified(HERE / name, DEPLOY_DOC_DIR / name) and ok
    return ok


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--no-deploy", action="store_true", help="生成と検査だけ行い、配置しない")
    ap.add_argument("--expect-identical", action="store_true",
                    help="生成物が配置済みとバイト一致しなければ終了コード 1")
    args = ap.parse_args()

    generated, expected_sections = generate()
    say("[INFO] 生成: %s" % generated)

    ok = check_shape(generated, expected_sections)
    ok = check_syntax(generated) and ok
    if not ok:
        say("[ERROR] 検査で落ちたので配置しない")
        return 1

    identical = compare(generated)
    if args.expect_identical and not identical:
        say("[ERROR] --expect-identical: 配置済みと一致しない")
        return 1

    if args.no_deploy:
        say("[INFO] --no-deploy: 配置しない")
        return 0

    if not deploy(generated):
        return 1
    say("[INFO] 配置完了。UI 定義を変えた場合は AviUtl2 の再起動が必要")
    return 0


if __name__ == "__main__":
    sys.exit(main())
