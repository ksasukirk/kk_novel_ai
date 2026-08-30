#!/usr/bin/env python3
"""IPC 写作测试：自动新建作品 → 写章纲 → IPC 拆节拍 + 续写。
代码路径: kk_novel_ai/scripts/ipc_writing_test.py
"""
from __future__ import annotations

import json
import os
import socket
import subprocess
import sys
import time
import uuid
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXE = ROOT / "dist" / "kk_novel_ai_test.exe"
if not EXE.exists():
    EXE = ROOT / "dist" / "kk_novel_ai_0.1.68.exe"
NOVELS = ROOT / "dist" / "novels"
APP_DATA = Path(os.environ.get("APPDATA", "")) / "kk_novel_ai" / "ipc.json"
OUT_JSON = ROOT / "dist" / "ipc_writing_test_result.json"

CHAPTER_SUMMARY = (
    "暑假第三周，表妹乐乐来 kk 家暂住。"
    "午后客厅：乐乐拒写暑假作业，kk 要求先写两页数学；"
    "双方口头争执升级，本节停在僵持，不写体罚。"
)


def run_cli(args: list[str], timeout: float = 30) -> int:
    if not EXE.exists():
        raise SystemExit(f"未找到构建产物: {EXE}")
    proc = subprocess.run(
        [str(EXE), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        cwd=str(ROOT / "dist"),
    )
    return proc.returncode


def setup_fresh_project() -> tuple[str, str]:
    """新建作品并写入章纲，返回 (project_root, chapter_id)。"""
    stamp = datetime.now().strftime("%m%d_%H%M%S")
    folder = NOVELS / f"IPC测试_{stamp}"
    folder.mkdir(parents=True, exist_ok=True)
    root = str(folder).replace("\\", "/")
    title = f"IPC测试_{stamp}"

    print(f"=== CLI 新建作品: {title} ===")
    rc = run_cli(["project", "create", root, "--title", title])
    project_json = folder / "project.json"
    if rc != 0 or not project_json.exists():
        raise RuntimeError(f"建项失败 (exit={rc})，目录: {folder}")

    data = json.loads(project_json.read_text(encoding="utf-8"))
    chapters = data.get("chapters") or []
    if not chapters:
        raise RuntimeError("建项成功但无章节")
    chapter_id = chapters[0]["id"]
    chapters[0]["summary"] = CHAPTER_SUMMARY
    data["chapters"] = chapters
    data["updated_at"] = datetime.now().astimezone().isoformat()
    project_json.write_text(
        json.dumps(data, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"作品路径: {root}")
    print(f"章节 ID: {chapter_id}")
    print(f"已写入章纲 summary")
    return root, chapter_id


def read_chapter_text(project_root: str) -> str:
    root = Path(project_root.replace("/", "\\"))
    ch_dir = root / "chapters"
    if not ch_dir.is_dir():
        return ""
    parts: list[str] = []
    for md in sorted(ch_dir.glob("*.md")):
        parts.append(md.read_text(encoding="utf-8", errors="replace"))
    branch = root / "chapters" / "branch.json"
    if branch.exists():
        parts.append(branch.read_text(encoding="utf-8", errors="replace"))
    return "\n".join(parts)


def has_beats_json_leak(text: str) -> bool:
    t = text or ""
    return ('"beats"' in t and '"reason"' in t) or t.strip().startswith('{"reason"')


def load_endpoint() -> dict:
    if not APP_DATA.exists():
        raise SystemExit(
            f"缺少 IPC 端点 {APP_DATA}，请先启动 GUI：\n  Start-Process {EXE}"
        )
    return json.loads(APP_DATA.read_text(encoding="utf-8"))


def ipc_call(ep: dict, payload: dict, collect_chunks: bool = False) -> dict:
    addr = (ep["host"], int(ep["port"]))
    body = dict(payload)
    body.setdefault("token", ep["token"])
    body.setdefault("id", str(uuid.uuid4()))
    line = (json.dumps(body, ensure_ascii=False) + "\n").encode("utf-8")

    chunks: list[str] = []
    with socket.create_connection(addr, timeout=5) as sock:
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


def main() -> int:
    project_root, chapter_id = setup_fresh_project()
    ep = load_endpoint()

    print("\n=== IPC gui_status ===")
    st = ipc_call(ep, {"cmd": "gui_status"})
    print(json.dumps(st, ensure_ascii=False, indent=2))

    ipc_call(
        ep,
        {
            "cmd": "project_focus",
            "root": project_root,
            "chapter_id": chapter_id,
        },
    )
    print(f"已通知 GUI 聚焦: {project_root}")

    print("\n=== IPC writing_run: outline_to_beats ===")
    beats_req = {
        "cmd": "writing_run",
        "request": {
            "project_root": project_root,
            "chapter_id": chapter_id,
            "task": "outline_to_beats",
            "instruction": CHAPTER_SUMMARY,
            "selection": "",
        },
        "apply": "none",
        "stream_chunks": True,
    }
    beats_res = ipc_call(ep, beats_req, collect_chunks=True)
    print("\n--- outline_to_beats 响应 ---")
    print(
        json.dumps(
            {k: v for k, v in beats_res.items() if k != "_stream_text"},
            ensure_ascii=False,
            indent=2,
        )
    )
    raw = beats_res.get("_stream_text") or beats_res.get("text") or ""
    if raw.strip():
        print("\n--- 模型原文（节选 800 字）---")
        print(raw[:800])

    time.sleep(2)
    leak_after_beats = has_beats_json_leak(read_chapter_text(project_root))
    print(
        f"\n=== 拆节拍后正文检查: {'失败(JSON 泄漏)' if leak_after_beats else '通过(无 JSON 写入正文)'} ==="
    )

    print("\n=== IPC writing_run: continue（模拟单节拍） ===")
    cont_instr = (
        "【按纲测试 1/3】只写本节拍：乐乐赖在沙发上拒写数学，kk 反复要求并发生口角；"
        "场景留在客厅，写到规定字数后停，禁止体罚与跳拍。"
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
    print("\n--- continue 响应 ---")
    print(
        json.dumps(
            {k: v for k, v in cont_res.items() if k != "_stream_text"},
            ensure_ascii=False,
            indent=2,
        )
    )
    text = cont_res.get("text") or cont_res.get("_stream_text") or ""
    if text.strip():
        print(f"\n--- 生成正文 {len(text)} 字（节选 600）---")
        print(text[:600])
    else:
        print("\n未收到正文，请检查 LM Studio 是否在运行。")

    time.sleep(3)
    final_chapter = read_chapter_text(project_root)
    leak_final = has_beats_json_leak(final_chapter)
    prose_len = len(final_chapter.strip())
    print(
        f"\n=== 最终正文检查: JSON泄漏={'是' if leak_final else '否'} | 章节文本约 {prose_len} 字 ==="
    )

    OUT_JSON.write_text(
        json.dumps(
            {
                "project_root": project_root,
                "chapter_id": chapter_id,
                "chapter_summary": CHAPTER_SUMMARY,
                "verify": {
                    "leak_after_beats": leak_after_beats,
                    "leak_final": leak_final,
                    "chapter_text_len": prose_len,
                },
                "gui_status": st,
                "outline_to_beats": {
                    k: v for k, v in beats_res.items() if not k.startswith("_")
                },
                "continue": {k: v for k, v in cont_res.items() if not k.startswith("_")},
                "continue_preview": (text or "")[:2000],
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"\n完整结果已写入: {OUT_JSON}")
    if leak_after_beats or leak_final:
        print("\n[FAIL] 检测到 beats JSON 写入正文，修复未生效。")
        return 1
    print("\n[PASS] IPC 测试通过：outline_to_beats 未污染正文。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
