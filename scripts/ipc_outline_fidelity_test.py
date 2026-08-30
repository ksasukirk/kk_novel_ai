#!/usr/bin/env python3
"""大纲忠实度测试：用户全书大纲 → outline_to_chapters，核对关键锚点是否保留。
代码路径: kk_novel_ai/scripts/ipc_outline_fidelity_test.py
"""
from __future__ import annotations

import json
import os
import re
import socket
import subprocess
import sys
import time
import uuid
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "dist"
NOVELS = DIST / "novels"
APP_DATA = Path(os.environ.get("APPDATA", "")) / "kk_novel_ai" / "ipc.json"
OUT_JSON = DIST / "ipc_outline_fidelity_test_result.json"

# 故意写死、可核对的锚点（含独特词，便于发现模型跑偏）
BOOK_OUTLINE = (
    "三章短篇《青瓷灯》。"
    "第一章：雨夜旧书店，女主沈青瓷独自盘点库存，发现一本写着自己名字的手帐；"
    "禁止写成乐乐/表妹线。"
    "第二章：次日中午，青瓷拿着手帐去见旧友周渡，两人只谈手帐来历，不谈恋爱。"
    "第三章：黄昏江边，青瓷把手帐沉入水中，留下一句「别再找我」作为结尾钩子。"
)

ANCHORS = [
    "沈青瓷",
    "旧书店",
    "手帐",
    "周渡",
    "江边",
    "别再找我",
]
FORBIDDEN = [
    "乐乐",
    "表妹",
    "暑假作业",
    "娜娜",
]


def find_exe() -> Path:
    for p in [DIST / "kk_novel_ai_test.exe", DIST / "kk_novel_ai.exe"]:
        if p.exists():
            return p
    cands = sorted(DIST.glob("kk_novel_ai_*.exe"), key=lambda x: x.stat().st_mtime, reverse=True)
    if cands:
        return cands[0]
    raise SystemExit("未找到 exe，请先编译")


def run_cli(exe: Path, args: list[str], timeout: float = 60) -> subprocess.CompletedProcess:
    return subprocess.run(
        [str(exe), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        cwd=str(DIST),
    )


def load_endpoint() -> dict | None:
    if not APP_DATA.exists():
        return None
    try:
        return json.loads(APP_DATA.read_text(encoding="utf-8"))
    except Exception:
        return None


def wait_ipc(timeout: float = 90) -> dict:
    deadline = time.time() + timeout
    while time.time() < deadline:
        ep = load_endpoint()
        if ep and ep.get("host") and ep.get("port") and ep.get("token"):
            try:
                with socket.create_connection((ep["host"], int(ep["port"])), timeout=1):
                    return ep
            except OSError:
                pass
        time.sleep(0.5)
    raise SystemExit(f"等待 IPC 超时: {APP_DATA}")


def ipc_call(ep: dict, payload: dict, collect_chunks: bool = False, timeout: float = 300) -> dict:
    body = dict(payload)
    body.setdefault("token", ep["token"])
    body.setdefault("id", str(uuid.uuid4()))
    line = (json.dumps(body, ensure_ascii=False) + "\n").encode("utf-8")
    chunks: list[str] = []
    with socket.create_connection((ep["host"], int(ep["port"])), timeout=10) as sock:
        sock.settimeout(timeout)
        sock.sendall(line)
        sock_file = sock.makefile("r", encoding="utf-8")
        while True:
            raw = sock_file.readline()
            if not raw:
                raise RuntimeError("IPC 提前关闭")
            msg = json.loads(raw)
            if msg.get("type") == "chunk":
                if collect_chunks:
                    chunks.append(msg.get("delta") or "")
                    sys.stderr.write(msg.get("delta") or "")
                    sys.stderr.flush()
                continue
            if collect_chunks:
                msg["_stream_text"] = "".join(chunks)
            return msg


def parse_chapters(text: str) -> list[dict]:
    raw = (text or "").strip()
    fence = re.search(r"```(?:json)?\s*([\s\S]*?)```", raw, re.I)
    if fence:
        raw = fence.group(1).strip()
    start, end = raw.find("{"), raw.rfind("}")
    if start < 0 or end <= start:
        return []
    try:
        data = json.loads(raw[start : end + 1])
    except json.JSONDecodeError:
        return []
    out = []
    for item in data.get("chapters") or []:
        if not isinstance(item, dict):
            continue
        out.append(
            {
                "title": str(item.get("title") or "").strip(),
                "summary": str(item.get("summary") or "").strip(),
                "must_do": str(item.get("must_do") or "").strip(),
            }
        )
    return [c for c in out if c["title"] or c["summary"]]


def setup_project(exe: Path) -> tuple[str, str]:
    stamp = datetime.now().strftime("%m%d_%H%M%S")
    folder = NOVELS / f"大纲忠实度_{stamp}"
    folder.mkdir(parents=True, exist_ok=True)
    root = str(folder).replace("\\", "/")
    proc = run_cli(exe, ["project", "create", root, "--title", f"大纲忠实度_{stamp}"])
    pj = folder / "project.json"
    if proc.returncode != 0 or not pj.exists():
        raise RuntimeError(f"建项失败: {proc.stderr}")
    data = json.loads(pj.read_text(encoding="utf-8"))
    chapter_id = data["chapters"][0]["id"]
    data["book_outline"] = BOOK_OUTLINE
    # 清空风格里可能带的默认修辞句，减少干扰
    data["style"] = "冷静短句，少修辞。"
    data["updated_at"] = datetime.now().astimezone().isoformat()
    pj.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return root, chapter_id


def score_fidelity(plan: list[dict]) -> dict:
    blob = "\n".join(f"{c['title']}\n{c['summary']}\n{c['must_do']}" for c in plan)
    # 判定「串戏」时去掉禁止句里的点名，避免「不得写成乐乐」误报
    blob_for_ban = re.sub(r"(禁止|不得|不要)[^。；;\n]{0,40}", "", blob)
    hit = [a for a in ANCHORS if a in blob]
    miss = [a for a in ANCHORS if a not in blob]
    leak = [f for f in FORBIDDEN if f in blob_for_ban]
    chapter_ok = 2 <= len(plan) <= 4
    return {
        "chapter_count": len(plan),
        "chapter_count_ok": chapter_ok,
        "anchors_hit": hit,
        "anchors_miss": miss,
        "forbidden_leak": leak,
        "anchor_hit_rate": round(len(hit) / max(len(ANCHORS), 1), 3),
        "ok": chapter_ok and len(miss) <= 1 and not leak,
    }


def main() -> int:
    exe = find_exe()
    print(f"EXE: {exe}")
    print(f"输入大纲:\n{BOOK_OUTLINE}\n")

    ep = load_endpoint()
    ready = False
    if ep and ep.get("host") and ep.get("port"):
        try:
            with socket.create_connection((ep["host"], int(ep["port"])), timeout=1):
                ready = True
                print(f"复用 GUI IPC {ep['host']}:{ep['port']}")
        except OSError:
            ready = False
    if not ready:
        print("启动 GUI…")
        subprocess.Popen([str(exe)], cwd=str(DIST), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        ep = wait_ipc(90)
        print(f"IPC 就绪 {ep['host']}:{ep['port']}")

    root, chapter_id = setup_project(exe)
    print(f"作品: {root}")
    ipc_call(ep, {"cmd": "project_focus", "root": root, "chapter_id": chapter_id})

    print("\n=== outline_to_chapters ===")
    res = ipc_call(
        ep,
        {
            "cmd": "writing_run",
            "request": {
                "project_root": root,
                "chapter_id": chapter_id,
                "task": "outline_to_chapters",
                "instruction": "",
                "selection": "",
            },
            "apply": "none",
            "stream_chunks": True,
        },
        collect_chunks=True,
    )
    raw = res.get("_stream_text") or res.get("text") or res.get("raw_text") or ""
    plan = parse_chapters(raw)
    print(f"\n\n解析章数: {len(plan)}")
    for i, c in enumerate(plan, 1):
        print(f"  {i}. {c['title']}")
        print(f"     {c['summary']}")

    fidelity = score_fidelity(plan)
    print("\n=== 忠实度 ===")
    print(json.dumps(fidelity, ensure_ascii=False, indent=2))

    out = {
        "ok": bool(fidelity.get("ok")),
        "book_outline": BOOK_OUTLINE,
        "anchors": ANCHORS,
        "forbidden": FORBIDDEN,
        "chapter_plan": plan,
        "fidelity": fidelity,
        "raw_preview": raw[:4000],
        "ipc": {k: v for k, v in res.items() if k != "_stream_text"},
        "project_root": root,
    }
    OUT_JSON.write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\n结果: {OUT_JSON}")
    if out["ok"]:
        print("\n[PASS] 大纲忠实度测试通过")
        return 0
    print("\n[FAIL] 拆章偏离原大纲意愿（锚点缺失或串入禁止剧情）")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
