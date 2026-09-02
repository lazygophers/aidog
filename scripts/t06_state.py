"""票 06 批量迁移工具（临时，交付前删除）。

把命令签名里的 `db: State<'_, Db>` 参数删掉，并在函数体首行补 `let db = aidog_ctx::db();`。
只做机械改写，编译错误由 cargo 兜底。
"""

import re
import sys
import pathlib

PARAM = re.compile(r"(?:\n)?[ \t]*(\w+)\s*:\s*(?:tauri::)?State<\s*'_\s*,\s*Db\s*>\s*,?")


def transform(path):
    p = pathlib.Path(path)
    text = p.read_text()
    i = 0
    changed = 0
    while True:
        m = re.search(r"pub (?:async )?fn (\w+)\s*\(", text[i:])
        if not m:
            break
        popen = i + m.end() - 1
        depth = 0
        j = popen
        while j < len(text):
            if text[j] == "(":
                depth += 1
            elif text[j] == ")":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        params = text[popen + 1 : j]
        names = [mm.group(1) for mm in PARAM.finditer(params)]
        if not names:
            i = j
            continue
        newparams = PARAM.sub("", params)
        newparams = re.sub(r"^\s*,", "", newparams)
        newparams = re.sub(r",\s*,", ",", newparams)
        newparams = newparams.rstrip()
        newparams = re.sub(r",\s*$", "", newparams)
        if not newparams.strip():
            newparams = ""
        k = text.index("{", j)
        lets = "".join(f"\n    let {n} = aidog_ctx::db();" for n in names)
        text = text[: popen + 1] + newparams + text[j : k + 1] + lets + text[k + 1 :]
        changed += 1
        i = popen + 1 + len(newparams) + len(lets)
    if changed:
        p.write_text(text)
    return changed


total = 0
for f in sys.argv[1:]:
    c = transform(f)
    if c:
        print(f, c)
    total += c
print("fns changed:", total)
