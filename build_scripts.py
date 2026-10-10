#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""FlowType_H のスクリプト（`@FlowType_H.anm2`）だけを生成・検査・配置する。

    python AI/plugins/FlowType_H/build_scripts.py                    # 生成 → 検査 → 配置
    python AI/plugins/FlowType_H/build_scripts.py --no-deploy        # 生成と検査だけ
    python AI/plugins/FlowType_H/build_scripts.py --no-deploy --expect-identical
                                                                     # 配置済みとバイト一致を確かめる

Korarei 氏の astra（**0.7.1 以降**。色の初期値 `nil` を通すのは 0.7.1 から。`astra.exe`、設定形式 version 2）の
`astra build effect --release` を呼ぶ。`astra.toml` にはスクリプトのビルドだけを書いてある。

**`Script/FlowType_H/@FlowType_H.anm2` は生成物。直接編集しない。**
正本は `scripts/effects/*.lua`（`motion.lua` ほか。`#include` される `utilities.lua` など）。

手順
----

1. `build/effect/release/@FlowType_H.anm2` へ生成する（`build/` は `.gitignore` 対象。
   同じ場所に出る多言語化のひな形 `Default.FlowType_H.aul2` は使わない）
2. 形を検査する: UTF-8 BOM 無し、先頭が `@モーション`、セクションが astra.toml の targets と同数
3. `AI/tools/check_lua_syntax.py` に通す（マルチセクションをセクション単位で luaJIT に読ませる）
4. 配置済みと比べる（一致 / 相違を出す。`--expect-identical` なら相違で終了コード 1）
5. 配置する（`--no-deploy` で省略）
   - `Script/FlowType_H/@FlowType_H.anm2`
   - `Preset/` へ `presets/*.preset`
   - `Script/FlowType_H/` へ `*.md` と `LICENSE`（aviutl2.toml の artifacts と同じ）
   配置後に読み戻してバイト一致を確かめる

UI 定義（`--track@` `--select@` など）を変えたときは、配置しても
**AviUtl2 の再起動まで反映されない**（「キャッシュを破棄」では足りない）。

終了コード: 0 = 成功 / 1 = 検査で落ちた / 2 = 実行できない（astra が無い等）
"""

import argparse
import io
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent                   # AI/plugins/FlowType_H
ROOT = HERE.parents[2]                                   # C:\ProgramData\aviutl2
BUILD_ID = "effect"
OUTPUT = HERE / "build" / BUILD_ID / "release" / "@FlowType_H.anm2"
ASTRA_MIN = (0, 7, 1)
DEPLOY_SCRIPT = ROOT / "Script" / "FlowType_H" / "@FlowType_H.anm2"
DEPLOY_DOC_DIR = ROOT / "Script" / "FlowType_H"
DEPLOY_PRESET_DIR = ROOT / "Preset"
CHECK_SYNTAX = ROOT / "AI" / "tools" / "check_lua_syntax.py"
DOCS = ("CHANGELOG.md", "README.md", "THIRD_PARTY_LICENSES.md", "LICENSE")

OUT = io.open(1, "w", encoding="utf-8", closefd=False)


def say(text=""):
    OUT.write(text + "\n")
    OUT.flush()


def find_astra():
    """PATH 上の astra が 0.7.1 以降であることを確かめて、そのパスを返す。"""
    exe = shutil.which("astra")
    if not exe:
        say("[ERROR] astra が PATH に無い（0.7.1 以降の astra.exe を入れる）")
        sys.exit(2)
    r = subprocess.run([exe, "--version"], capture_output=True, text=True, encoding="utf-8")
    m = re.search(r"(\d+)\.(\d+)\.(\d+)", r.stdout)
    if r.returncode != 0 or not m or tuple(map(int, m.groups())) < ASTRA_MIN:
        say("[ERROR] astra が 0.7.1 より古いか、版が読めない（%s: %s）"
            % (exe, (r.stdout or r.stderr).strip()))
        sys.exit(2)
    return exe


def generate():
    """astra build で release のスクリプトを生成し、生成物のパスとセクションの期待数を返す。"""
    exe = find_astra()
    with open(HERE / "astra.toml", "rb") as f:
        expected_sources = len(tomllib.load(f)["builds"][BUILD_ID]["targets"])

    if OUTPUT.exists():
        OUTPUT.unlink()                                  # 前回の生成物を今回のものと取り違えない
    r = subprocess.run([exe, "build", BUILD_ID, "--release"], cwd=str(HERE),
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    log = re.sub(r"\x1b\[[0-9;]*m", "", (r.stdout or "") + (r.stderr or ""))
    for ln in log.splitlines():
        if ln.strip():
            say("[astra] " + ln.strip())
    if r.returncode != 0 or not OUTPUT.is_file():
        say("[ERROR] astra build が失敗した（終了コード %d）" % r.returncode)
        sys.exit(1)
    return OUTPUT, expected_sources


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
        say("[ERROR] セクション数 %d（astra.toml の targets は %d）: %s"
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
    # unified_diff の出力から数えると、Lua のコメント行（-- で始まる）を消した行が
    # 見出しの --- と見分けられずに落ちるので、opcodes から数える
    changed = sum(max(i2 - i1, j2 - j1)
                  for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, lb, la, autojunk=False).get_opcodes()
                  if tag != "equal")
    note = "差分 %d 行" % changed if changed else "行の中身は同じ。改行コードなどの違い"
    say("[INFO] 配置済みと相違（生成 %d バイト / 配置済み %d バイト、%s）" % (len(a), len(b), note))
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
