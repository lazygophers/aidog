# PostToolUse(Bash) — 监测 task.py start/archive, 自动管 worktree
# 不改 task.py, 仅在其执行后做 git worktree 副作用

import json, os, re, subprocess, sys

d = json.load(sys.stdin)
cmd = (d.get("tool_input") or {}).get("command", "")
cwd = d.get("cwd") or os.getcwd()

if not os.path.isdir(os.path.join(cwd, ".trellis")):
    sys.exit(0)

def task_id():
    # task.py current 拿当前 task 目录名
    r = subprocess.run(["python3", ".trellis/scripts/task.py", "current"],
                       capture_output=True, text=True, cwd=cwd, timeout=5)
    return os.path.basename(r.stdout.strip()) if r.returncode == 0 and r.stdout.strip() else None

# create / start → 建 worktree (创建任务即切, 步骤 1)
if re.search(r"task\.py\s+(create|start)\b", cmd):
    tid = task_id()
    if tid:
        wt = os.path.join(cwd, ".trellis", "worktrees", tid)
        if not os.path.isdir(wt):
            subprocess.run(["git","-C",cwd,"worktree","add",wt,"-b",f"trellisx-{tid}"],
                           capture_output=True, timeout=15)
        print(json.dumps({"hookSpecificOutput":{"hookEventName":"PostToolUse",
            "additionalContext":f"trellisx: worktree 已建于 .trellis/worktrees/{tid}。源码改动写该路径内, 或 EnterWorktree 切入。"}}))

# archive → 检干净则销毁 worktree
elif re.search(r"task\.py\s+archive\b", cmd):
    m = re.search(r"archive\s+(\S+)", cmd)
    tid = os.path.basename(m.group(1)) if m else None
    if tid:
        wt = os.path.join(cwd, ".trellis", "worktrees", tid)
        if os.path.isdir(wt):
            st = subprocess.run(["git","-C",wt,"status","--porcelain"],
                                capture_output=True, text=True, timeout=5)
            if st.stdout.strip():  # 脏 → 不销毁, 警告
                print(json.dumps({"hookSpecificOutput":{"hookEventName":"PostToolUse",
                    "additionalContext":f"trellisx: worktree .trellis/worktrees/{tid} 有未提交改动, 未销毁。先合并 (git -C {wt} ...) + 手动 git worktree remove。"}}))
            else:  # 干净 → 销毁
                subprocess.run(["git","-C",cwd,"worktree","remove",wt], capture_output=True, timeout=10)
                subprocess.run(["git","-C",cwd,"branch","-D",f"trellisx-{tid}"], capture_output=True, timeout=5)
sys.exit(0)
