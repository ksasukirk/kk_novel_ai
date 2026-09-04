#!/usr/bin/env python3
"""
DeepSeek 大纲拆章 + 按纲写完全书（短篇基准测）
代码路径: kk_novel_ai/scripts/deepseek_outline_pipeline.py
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "src-tauri" / "target" / "debug" / "kk_novel_cli.exe"
OUT_DIR = ROOT / "outputs"
NOVELS = ROOT / "dist" / "novels"
BOOK_TITLE = f"DeepSeekCacheTest_{datetime.now().strftime('%m%d_%H%M')}"
BOOK_OUTLINE = """书名：雨棚下的约定
类型：都市情感短篇，三章完结。
主角：娜娜（女，大学生）、kk（男，程序员）。
基调：雨夜暧昧、克制又黏，不写未成年，不写暴力。
结构：
第一章：暴雨延误班车，两人挤在同一公交站雨棚；娜娜认出 kk 是以前辅导过她编程的学长；尴尬寒暄里透出旧情。
第二章：雨停后去便利店躲空调；聊到工作压力与失眠；kk 把外套借给她，手背相触；约定周末一起改简历。
第三章：周末在咖啡馆；娜娜拿到实习 offer；两人确认想继续见面；结尾雨又下了，两人共伞离开，钩子是下一场约会地点未定。
硬规则：每章有明确冲突与收束；人称性别锁；禁止复读。
"""


def run_cli(args: list[str], timeout: int = 900) -> dict:
    cmd = [str(CLI), *args]
    print(f"[CLI] {' '.join(args[:8])}{'...' if len(args) > 8 else ''}", flush=True)
    p = subprocess.run(
        cmd,
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
    )
    out = (p.stdout or "").strip() or (p.stderr or "").strip()
    if p.returncode != 0 and not out:
        raise RuntimeError(f"CLI failed rc={p.returncode}: {p.stderr}")
    # 取最后一段 JSON
    text = p.stdout or ""
    if not text.strip():
        text = p.stderr or ""
    # 可能混有日志，从最后一个 { 起解析
    start = text.rfind("{")
    if start < 0:
        raise RuntimeError(f"No JSON in CLI output:\n{text[:2000]}")
    try:
        data = json.loads(text[start:])
    except json.JSONDecodeError:
        # 尝试从第一个 { 到末尾
        start = text.find("{")
        data = json.loads(text[start:])
    if isinstance(data, dict) and data.get("ok") is False:
        raise RuntimeError(json.dumps(data, ensure_ascii=False)[:2000])
    return data


def parse_chapters(text: str) -> list[dict]:
    raw = (text or "").strip()
    fence = re.search(r"```(?:json)?\s*([\s\S]*?)```", raw, re.I)
    body = fence.group(1).strip() if fence else raw
    start, end = body.find("{"), body.rfind("}")
    if start < 0 or end <= start:
        return []
    data = json.loads(body[start : end + 1])
    chapters = []
    for item in data.get("chapters") or []:
        if not isinstance(item, dict):
            continue
        chapters.append(
            {
                "title": str(item.get("title") or "").strip(),
                "summary": str(item.get("summary") or "").strip(),
                "must_do": str(item.get("must_do") or item.get("mustDo") or "").strip(),
            }
        )
    return [c for c in chapters if c["title"] or c["summary"]][:30]


def wrap_instruction(ch: dict) -> str:
    title = ch.get("title") or "本章"
    summary = ch.get("summary") or ""
    parts = [
        f"【按纲生成 · 整章一次写完】章节「{title}」。本章正文只生成一整段完整内容，不要拆小节、不要分段标拍。",
        "须达到或超出规定字数后再停；覆盖章纲中的冲突、推进与结尾钩子；承接上章收束（若有）。",
        f"本章纲：\n{summary}",
    ]
    if ch.get("must_do"):
        parts.append(f"必达：{ch['must_do']}")
    return "\n".join(parts)


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    NOVELS.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    report_path = OUT_DIR / f"deepseek_pipeline_{stamp}.json"
    root = NOVELS / BOOK_TITLE

    timeline: list[dict] = []
    t0 = time.time()

    print(f"== create project: {root}", flush=True)
    create = run_cli(["project", "create", str(root), "--title", BOOK_TITLE])
    timeline.append({"step": "project_create", "ok": True, "keys": list(create.keys())})

    pj = root / "project.json"
    if not pj.exists():
        raise RuntimeError("project.json missing after create")
    disk = json.loads(pj.read_text(encoding="utf-8"))
    disk["book_outline"] = BOOK_OUTLINE
    disk["title"] = BOOK_TITLE
    disk["style"] = "第三人称；细腻感官；对白自然；克制暧昧。"
    pj.write_text(json.dumps(disk, ensure_ascii=False, indent=2), encoding="utf-8")
    timeline.append({"step": "save_book_outline", "chars": len(BOOK_OUTLINE)})

    # Ensure at least one chapter
    disk = json.loads(pj.read_text(encoding="utf-8"))
    chapters = disk.get("chapters") or []
    if not chapters:
        ch_create = run_cli(["chapter", "create", str(root), "第一章", ""])
        timeline.append({"step": "chapter_create_placeholder", "result_keys": list(ch_create.keys())})
        disk = json.loads(pj.read_text(encoding="utf-8"))
        chapters = disk.get("chapters") or []
    chapter0 = chapters[0]["id"]

    print("== outline_to_chapters", flush=True)
    split = run_cli(
        [
            "writing",
            "run",
            str(root),
            chapter0,
            "outline_to_chapters",
            "--instruction",
            "",
            "--offline",
            "--apply",
            "none",
        ],
        timeout=600,
    )
    split_text = split.get("raw_text") or split.get("text") or ""
    plan = parse_chapters(split_text)
    if not plan:
        raise RuntimeError(f"拆章失败，原文前500字：{split_text[:500]}")
    timeline.append(
        {
            "step": "outline_to_chapters",
            "chapter_count": len(plan),
            "usage": split.get("usage"),
            "cost_cny": split.get("cost_cny"),
            "model_used": split.get("model_used"),
            "titles": [c["title"] for c in plan],
        }
    )
    (OUT_DIR / f"split_raw_{stamp}.txt").write_text(split_text, encoding="utf-8")

    # Apply chapter plan: update first empty chapter, create rest
    disk = json.loads(pj.read_text(encoding="utf-8"))
    existing = disk.get("chapters") or []
    applied_ids: list[str] = []

    # Update first reusable empty chapter
    first = existing[0]
    first_title = plan[0]["title"] or "第一章"
    first_summary = plan[0]["summary"]
    # chapter update via writing project.json + chapter_update_meta CLI if exists
    try:
        run_cli(
            [
                "chapter",
                "update",
                str(root),
                first["id"],
                "--title",
                first_title,
                "--summary",
                first_summary,
                "--status",
                "draft",
            ]
        )
    except Exception:
        # direct file update
        disk = json.loads(pj.read_text(encoding="utf-8"))
        disk["chapters"][0]["title"] = first_title
        disk["chapters"][0]["summary"] = first_summary
        if plan[0].get("must_do"):
            disk["chapters"][0]["must_do"] = plan[0]["must_do"]
        disk["chapters"][0]["status"] = "draft"
        pj.write_text(json.dumps(disk, ensure_ascii=False, indent=2), encoding="utf-8")
    applied_ids.append(first["id"])

    for i, ch in enumerate(plan[1:], start=2):
        title = ch["title"] or f"第{i}章"
        summary = ch["summary"]
        created = run_cli(["chapter", "create", str(root), title, "--summary", summary])
        # find id
        disk = json.loads(pj.read_text(encoding="utf-8"))
        # last chapter usually
        new_ch = None
        for c in disk.get("chapters") or []:
            if c.get("title") == title and c.get("id") not in applied_ids:
                new_ch = c
        if not new_ch:
            new_ch = (disk.get("chapters") or [])[-1]
        # ensure summary
        for c in disk["chapters"]:
            if c["id"] == new_ch["id"]:
                c["summary"] = summary
                if ch.get("must_do"):
                    c["must_do"] = ch["must_do"]
                c["status"] = "draft"
        pj.write_text(json.dumps(disk, ensure_ascii=False, indent=2), encoding="utf-8")
        applied_ids.append(new_ch["id"])

    timeline.append({"step": "apply_chapter_plan", "ids": applied_ids, "count": len(applied_ids)})

    # Write all chapters
    write_results = []
    disk = json.loads(pj.read_text(encoding="utf-8"))
    id_to_meta = {c["id"]: c for c in disk.get("chapters") or []}
    for cid in applied_ids:
        ch = id_to_meta.get(cid) or {"title": "?", "summary": ""}
        instr = wrap_instruction(ch)
        print(f"== continue {ch.get('title')}", flush=True)
        # clear body first
        run_cli(["chapter", "write", str(root), cid, "--content", ""])
        out = run_cli(
            [
                "writing",
                "run",
                str(root),
                cid,
                "continue",
                "--instruction",
                instr,
                "--offline",
                "--apply",
                "append",
            ],
            timeout=900,
        )
        text = out.get("raw_text") or out.get("text") or ""
        # mark done
        disk = json.loads(pj.read_text(encoding="utf-8"))
        for c in disk.get("chapters") or []:
            if c["id"] == cid:
                c["status"] = "done"
        pj.write_text(json.dumps(disk, ensure_ascii=False, indent=2), encoding="utf-8")
        item = {
            "chapter_id": cid,
            "title": ch.get("title"),
            "chars": len(text),
            "usage": out.get("usage"),
            "cost_cny": out.get("cost_cny"),
            "model_used": out.get("model_used"),
            "log_id": out.get("log_id"),
        }
        write_results.append(item)
        timeline.append({"step": "continue", **item})
        print(f"   chars={item['chars']} cost={item.get('cost_cny')} usage={item.get('usage')}", flush=True)

    # Chapter body stats
    body_stats = []
    total_chars = 0
    for cid in applied_ids:
        ch_path = root / "chapters" / f"{cid}.md"
        # find actual chapter file
        files = list((root / "chapters").glob("*.md")) if (root / "chapters").exists() else []
        content = ""
        # try chapter_read
        try:
            r = run_cli(["chapter", "read", str(root), cid])
            content = r.get("content") or ""
        except Exception:
            for f in files:
                if cid in f.name:
                    content = f.read_text(encoding="utf-8", errors="replace")
                    break
        n = len(re.sub(r"\s+", "", content))  # 去空白字数粗估
        n2 = len(content)
        body_stats.append({"chapter_id": cid, "chars_raw": n2, "chars_no_ws": n})
        total_chars += n2

    # usage summary
    try:
        usage = run_cli(["rpc", json.dumps({"cmd": "usage_summary", "root": str(root)}, ensure_ascii=False)])
    except Exception:
        try:
            usage = run_cli(["gen-log", "list", "50"])
        except Exception as e:
            usage = {"error": str(e)}

    # Also parse local ledger / gen_log for this project
    appdata = Path.home() / "AppData" / "Roaming" / "kk_novel_ai"
    ledger = {}
    ledger_path = appdata / "usage_ledger.json"
    if ledger_path.exists():
        try:
            ledger = json.loads(
                "".join(
                    ch if (ord(ch) >= 32 or ch in "\n\r\t") else " "
                    for ch in ledger_path.read_text(encoding="utf-8", errors="replace")
                )
            )
        except Exception as e:
            ledger = {"error": str(e)}

    gen_items = []
    gen_path = appdata / "gen_log.jsonl"
    if gen_path.exists():
        lines = gen_path.read_text(encoding="utf-8", errors="replace").splitlines()
        for line in lines[-200:]:
            line = line.strip()
            if not line:
                continue
            try:
                e = json.loads(line)
            except Exception:
                continue
            if str(root) in str(e.get("project_root") or ""):
                gen_items.append(e)

    report = {
        "book_title": BOOK_TITLE,
        "root": str(root),
        "elapsed_sec": round(time.time() - t0, 1),
        "chapter_plan": plan,
        "write_results": write_results,
        "body_stats": body_stats,
        "total_body_chars": total_chars,
        "timeline": timeline,
        "usage_summary_rpc": usage,
        "usage_ledger_global": {
            "total_prompt_tokens": ledger.get("total_prompt_tokens"),
            "total_completion_tokens": ledger.get("total_completion_tokens"),
            "total_prompt_cache_hit_tokens": ledger.get("total_prompt_cache_hit_tokens"),
            "total_prompt_cache_miss_tokens": ledger.get("total_prompt_cache_miss_tokens"),
            "total_cost_cny": ledger.get("total_cost_cny"),
            "total_calls": ledger.get("total_calls"),
            "project_bucket": (ledger.get("by_project") or {}).get(str(root)),
        },
        "gen_log_project": [
            {
                "ts": g.get("ts"),
                "task": g.get("task"),
                "model_used": g.get("model_used"),
                "chars_final": g.get("chars_final"),
                "cost_cny": g.get("cost_cny"),
                "usage": g.get("usage"),
            }
            for g in gen_items
        ],
    }
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"== report: {report_path}", flush=True)
    print(json.dumps({
        "chapters": len(plan),
        "total_body_chars": total_chars,
        "writes": write_results,
        "elapsed_sec": report["elapsed_sec"],
    }, ensure_ascii=False, indent=2), flush=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as e:
        print(f"FATAL: {e}", file=sys.stderr)
        raise
