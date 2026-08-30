#!/usr/bin/env python3
"""IPC 按纲拆章 + 续写生成测试。
流程：新建作品 → 写入 book_outline → outline_to_chapters → 落盘章纲 → continue
代码路径: kk_novel_ai/scripts/ipc_outline_pipeline_test.py
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
OUT_JSON = DIST / "ipc_outline_pipeline_test_result.json"

BOOK_OUTLINE = (
    "暑门短篇三章。"
    "第一章：表妹乐乐来 kk 家暂住，午后客厅拒写暑假作业，口角僵持。"
    "第二章：晚饭后 kk 监督乐乐补作业，气氛别扭但仍留下。"
    "第三章：次日清晨收束，乐乐答应写完再玩，留开放式钩子。"
)


def find_exe() -> Path:
    preferred = [
        DIST / "kk_novel_ai_test.exe",
        DIST / "kk_novel_ai.exe",
    ]
    for p in preferred:
        if p.exists():
            return p
    cands = sorted(DIST.glob("kk_novel_ai_*.exe"), key=lambda x: x.stat().st_mtime, reverse=True)
    if cands:
        return cands[0]
    raise SystemExit(f"未找到构建产物 exe，请先 python build.py --platform windows --no-bump")


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


def setup_project(exe: Path) -> tuple[str, str, Path]:
    stamp = datetime.now().strftime("%m%d_%H%M%S")
    folder = NOVELS / f"大纲流水线_{stamp}"
    folder.mkdir(parents=True, exist_ok=True)
    root = str(folder).replace("\\", "/")
    title = f"大纲流水线_{stamp}"

    print(f"=== CLI 新建作品: {title} ===")
    proc = run_cli(exe, ["project", "create", root, "--title", title])
    project_json = folder / "project.json"
    if proc.returncode != 0 or not project_json.exists():
        raise RuntimeError(
            f"建项失败 exit={proc.returncode}\nstdout={proc.stdout}\nstderr={proc.stderr}"
        )

    data = json.loads(project_json.read_text(encoding="utf-8"))
    chapters = data.get("chapters") or []
    if not chapters:
        raise RuntimeError("建项成功但无章节")
    chapter_id = chapters[0]["id"]
    data["book_outline"] = BOOK_OUTLINE
    data["updated_at"] = datetime.now().astimezone().isoformat()
    project_json.write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"作品路径: {root}")
    print(f"章节 ID: {chapter_id}")
    print("已写入 book_outline")
    return root, chapter_id, project_json


def load_endpoint() -> dict | None:
    if not APP_DATA.exists():
        return None
    try:
        return json.loads(APP_DATA.read_text(encoding="utf-8"))
    except Exception:
        return None


def wait_ipc(timeout: float = 45) -> dict:
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
    raise SystemExit(f"等待 IPC 超时（{timeout}s）: {APP_DATA}")


def ipc_call(ep: dict, payload: dict, collect_chunks: bool = False, timeout: float = 300) -> dict:
    addr = (ep["host"], int(ep["port"]))
    body = dict(payload)
    body.setdefault("token", ep["token"])
    body.setdefault("id", str(uuid.uuid4()))
    line = (json.dumps(body, ensure_ascii=False) + "\n").encode("utf-8")

    chunks: list[str] = []
    with socket.create_connection(addr, timeout=10) as sock:
        sock.settimeout(timeout)
        sock.sendall(line)
        sock_file = sock.makefile("r", encoding="utf-8")
        while True:
            raw = sock_file.readline()
            if not raw:
                raise RuntimeError("IPC 连接提前关闭")
            msg = json.loads(raw)
            if msg.get("type") == "chunk":
                delta = msg.get("delta") or ""
                if collect_chunks:
                    chunks.append(delta)
                    sys.stderr.write(delta)
                    sys.stderr.flush()
                continue
            if collect_chunks:
                msg["_stream_text"] = "".join(chunks)
            return msg


def parse_chapters(text: str) -> list[dict]:
    raw = (text or "").strip()
    if not raw:
        return []
    fence = re.search(r"```(?:json)?\s*([\s\S]*?)```", raw, re.I)
    if fence:
        raw = fence.group(1).strip()
    start = raw.find("{")
    end = raw.rfind("}")
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
        title = str(item.get("title") or "").strip()
        summary = str(item.get("summary") or "").strip()
        must_do = str(item.get("must_do") or "").strip()
        if title or summary:
            out.append({"title": title or "未命名章", "summary": summary, "must_do": must_do})
    return out[:30]


def apply_chapters(project_json: Path, plan: list[dict]) -> list[str]:
    """把拆章结果写入首章 + 追加后续章（简化：只更新首章 summary，其余 CLI chapter create）。"""
    data = json.loads(project_json.read_text(encoding="utf-8"))
    chapters = data.get("chapters") or []
    if not chapters or not plan:
        raise RuntimeError("无法落盘章列表")
    chapters[0]["title"] = plan[0]["title"]
    chapters[0]["summary"] = plan[0]["summary"]
    if plan[0].get("must_do"):
        chapters[0]["must_do"] = plan[0]["must_do"]
    data["chapters"] = chapters
    data["updated_at"] = datetime.now().astimezone().isoformat()
    project_json.write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return [chapters[0]["id"]]


def main() -> int:
    exe = find_exe()
    print(f"使用 EXE: {exe}")

    # 如无 IPC，拉起 GUI
    gui_proc = None
    ep = load_endpoint()
    ready = False
    if ep and ep.get("host") and ep.get("port"):
        try:
            with socket.create_connection((ep["host"], int(ep["port"])), timeout=1):
                print(f"复用已有 GUI IPC {ep['host']}:{ep['port']}")
                ready = True
        except OSError:
            ready = False
    if not ready:
        print("=== 启动 GUI ===")
        gui_proc = subprocess.Popen(
            [str(exe)],
            cwd=str(DIST),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        ep = wait_ipc(90)
        print(f"GUI IPC 就绪 {ep['host']}:{ep['port']}")

    project_root, chapter_id, project_json = setup_project(exe)

    st = ipc_call(ep, {"cmd": "gui_status"})
    print("gui_status:", json.dumps(st, ensure_ascii=False))

    ipc_call(
        ep,
        {"cmd": "project_focus", "root": project_root, "chapter_id": chapter_id},
    )
    print(f"已聚焦作品: {project_root}")

    print("\n=== IPC writing_run: outline_to_chapters ===")
    split_req = {
        "cmd": "writing_run",
        "request": {
            "project_root": project_root,
            "chapter_id": chapter_id,
            "task": "outline_to_chapters",
            "instruction": "",
            "selection": "",
        },
        "apply": "none",
        "stream_chunks": True,
    }
    split_res = ipc_call(ep, split_req, collect_chunks=True)
    raw = split_res.get("_stream_text") or split_res.get("text") or split_res.get("raw_text") or ""
    print("\n--- outline_to_chapters 响应摘要 ---")
    print(
        json.dumps(
            {k: v for k, v in split_res.items() if k not in ("_stream_text",) and not str(k).startswith("_")},
            ensure_ascii=False,
            indent=2,
        )[:2000]
    )
    if raw.strip():
        print("\n--- 拆章原文（节选 1000）---")
        print(raw[:1000])

    plan = parse_chapters(raw)
    print(f"\n解析到 {len(plan)} 章")
    for i, ch in enumerate(plan, 1):
        print(f"  {i}. {ch['title']} | summary={ch['summary'][:60]}…")

    ok_split = len(plan) >= 1
    if not ok_split:
        print("[FAIL] 未能解析出章节列表，请检查分析模型 / LM Studio")
        OUT_JSON.write_text(
            json.dumps(
                {
                    "ok": False,
                    "stage": "outline_to_chapters",
                    "project_root": project_root,
                    "split_res": {k: v for k, v in split_res.items() if k != "_stream_text"},
                    "raw_preview": raw[:3000],
                },
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        return 1

    apply_chapters(project_json, plan)
    # 重新聚焦以刷新 GUI 元数据
    ipc_call(
        ep,
        {"cmd": "project_focus", "root": project_root, "chapter_id": chapter_id},
    )

    print("\n=== IPC writing_run: continue（按首章纲写一节） ===")
    cont_instr = (
        f"【按纲生成测试】依据章纲只写开场一节："
        f"{plan[0]['summary'][:120]}；写到规定字数后停，禁止跳章。"
    )
    cont_req = {
        "cmd": "writing_run",
        "request": {
            "project_root": project_root,
            "chapter_id": chapter_id,
            "task": "continue",
            "instruction": cont_instr,
            "selection": "",
        },
        "apply": "none",
        "stream_chunks": True,
    }
    cont_res = ipc_call(ep, cont_req, collect_chunks=True)
    text = cont_res.get("text") or cont_res.get("_stream_text") or ""
    print(f"\n--- continue 生成约 {len(text)} 字（节选 600）---")
    print((text or "")[:600] or "（无正文，请检查写作模型）")

    ok_continue = len((text or "").strip()) >= 80
    result = {
        "ok": ok_split and ok_continue,
        "exe": str(exe),
        "project_root": project_root,
        "chapter_id": chapter_id,
        "book_outline": BOOK_OUTLINE,
        "chapter_plan": plan,
        "chapter_plan_count": len(plan),
        "continue_chars": len(text or ""),
        "continue_preview": (text or "")[:2000],
        "outline_to_chapters": {
            k: v for k, v in split_res.items() if k != "_stream_text"
        },
        "continue": {k: v for k, v in cont_res.items() if k != "_stream_text"},
    }
    OUT_JSON.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"\n结果已写入: {OUT_JSON}")

    if ok_split and ok_continue:
        print("\n[PASS] 拆章 + 生成测试通过")
        return 0
    print("\n[FAIL] 拆章或生成未达标")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
