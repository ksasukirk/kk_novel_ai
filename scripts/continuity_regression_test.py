#!/usr/bin/env python3
"""对照测试：用与「未命名小说17」相同的一句话大纲，走改后管线（拆章+写三章+章摘要），再评分。
代码路径: kk_novel_ai/scripts/continuity_regression_test.py
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "outputs"
NOVELS = ROOT / "dist" / "novels"
BOOK_OUTLINE = "乐乐想成为kk表哥的母狗"
CONTINUITY_MUST_NOT = (
    "禁止把上章已发生的用餐、入睡、出行写成尚未发生；"
    "禁止同一顿饭再开一桌；"
    "禁止改写已确立的亲属（谁是谁家的孩子、表哥表妹属哪一门）。"
)
CONTINUITY_WRITE_HINT = (
    "承接上章收束的时间地点与人物状态；"
    "上章已用餐或已吃西瓜则按饭后写，禁止喊开饭；亲属称谓以角色卡为准。"
)
SHORT_OUTLINE_MUST_NOT = (
    "一句话大纲：只推进该句已有动作；末章须留下未兑现的核心愿望/下场钩子，禁止假装全书已经写完。"
)


def find_cli() -> Path:
    env = os.environ.get("CARGO_TARGET_DIR")
    cands = []
    if env:
        cands.append(Path(env) / "debug" / "kk_novel_cli.exe")
        cands.append(Path(env) / "release" / "kk_novel_cli.exe")
    cands += [
        ROOT / "src-tauri" / "target" / "debug" / "kk_novel_cli.exe",
        ROOT / "src-tauri" / "target" / "release" / "kk_novel_cli.exe",
        ROOT / "target" / "debug" / "kk_novel_cli.exe",
    ]
    for p in cands:
        if p.exists():
            return p
    raise SystemExit("未找到 kk_novel_cli.exe，请先 cargo build --bin kk_novel_cli")


def run_cli(cli: Path, args: list[str], timeout: int = 900) -> dict:
    cmd = [str(cli), *args]
    print(f"[CLI] {' '.join(args[:10])}{'...' if len(args) > 10 else ''}", flush=True)
    p = subprocess.run(
        cmd,
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
    )
    text = (p.stdout or "").strip()
    err = (p.stderr or "").strip()
    if p.returncode != 0 and not text:
        raise RuntimeError(f"CLI rc={p.returncode} stderr={err[:2000]}")
    start = text.rfind("{")
    if start < 0:
        raise RuntimeError(f"CLI 无 JSON rc={p.returncode}\n{text[:2000]}")
    try:
        data = json.loads(text[start:])
    except json.JSONDecodeError:
        data = json.loads(text[text.find("{") :])
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
                "must_not": str(item.get("must_not") or item.get("mustNot") or "").strip(),
            }
        )
    return [c for c in chapters if c["title"] or c["summary"]][:30]


def compose_must_not(row: dict, outline: str) -> str:
    parts = []
    user = str((row or {}).get("must_not") or "").strip()
    if user:
        parts.append(user)
    compact = re.sub(r"\s+", "", outline or "")
    if outline and len(compact) < 80:
        parts.append(SHORT_OUTLINE_MUST_NOT)
    if not any("禁止把上章已发生的用餐" in p for p in parts):
        parts.append(CONTINUITY_MUST_NOT)
    return " ".join(parts)


def seed_title(outline: str) -> str:
    line = re.sub(r"\s+", "", (outline or "").strip().splitlines()[0])
    line = re.sub(r'[\\/:*?"<>|]', "", line)
    chars = list(line)
    return "".join(chars[:28] if len(chars) <= 28 else chars[:24])


def wrap_instruction(ch: dict) -> str:
    title = ch.get("title") or "本章"
    summary = ch.get("summary") or ""
    parts = [
        f"【按纲生成 · 整章一次写完】章节「{title}」。本章正文只生成一整段完整内容，不要拆小节、不要分段标拍。",
        "须达到或超出规定字数后再停；覆盖章纲中的冲突、推进与结尾钩子；承接上章收束（若有）。",
        CONTINUITY_WRITE_HINT,
        f"本章纲：\n{summary}",
    ]
    if ch.get("must_do"):
        parts.append(f"必达：{ch['must_do']}")
    if ch.get("must_not"):
        parts.append(f"禁止：{ch['must_not']}")
    return "\n".join(parts)


def read_chapter_body(cli: Path, root: Path, cid: str) -> str:
    r = run_cli(cli, ["chapter", "read", str(root), cid], timeout=60)
    return r.get("content") or ""


def compact_prefix(s: str, n: int = 48) -> str:
    return re.sub(r"\s+", "", s or "")[:n]


def analyze(root: Path, plan: list[dict], title: str) -> dict:
    pj = json.loads((root / "project.json").read_text(encoding="utf-8"))
    mem = {}
    mp = root / "memory.json"
    if mp.exists():
        mem = json.loads(mp.read_text(encoding="utf-8"))
    bodies = []
    for ch in pj.get("chapters") or []:
        fp = root / "chapters" / ch.get("file", "")
        text = fp.read_text(encoding="utf-8") if fp.exists() else ""
        bodies.append({"id": ch["id"], "title": ch.get("title"), "text": text, "meta": ch})

    joined = "\n".join(b["text"] for b in bodies)
    checks = []

    def add(key, ok, detail):
        checks.append({"id": key, "pass": bool(ok), "detail": detail})

    # 1 拆章：must_not / 时间 / 开局弧
    has_must_not = all(c.get("must_not") for c in plan) if plan else False
    time_words = ("饭前", "饭中", "饭后", "晚饭", "开饭", "翌日", "当晚")
    plan_has_time = any(any(w in (c.get("summary") or "") for w in time_words) for c in plan)
    last_sum = (plan[-1].get("summary") if plan else "") or ""
    last_unfulfilled = any(w in last_sum for w in ("钩子", "明晚", "未兑现", "尚未", "下次", "约"))
    add("split_must_not", has_must_not, f"拆章 {len(plan)} 章；must_not 全有={has_must_not}")
    add("split_time_in_summary", plan_has_time, "章纲是否写时间锁")
    add("split_opening_hook", last_unfulfilled, f"末章纲钩子：{last_sum[:80]}")

    # 2 书名
    add("title_from_outline", title == BOOK_OUTLINE or BOOK_OUTLINE in title, f"title={title}")

    # 3 晚饭回拨
    ch1 = bodies[0]["text"] if bodies else ""
    later = "\n".join(b["text"] for b in bodies[1:])
    ch1_ate = bool(re.search(r"晚饭|开饭|添饭|长桌|入席", ch1))
    rewind = bool(re.search(r"该吃饭了|还没开饭|快到饭点|喊人入席|开饭了", later))
    add("no_dinner_rewind", not (ch1_ate and rewind), f"第1章用餐={ch1_ate}；后章回拨开饭={rewind}")

    # 4 亲属对调
    kk_erjiu = bool(re.search(r"kk.{0,12}二舅家的(?:孩子|儿子)|二舅家的(?:孩子|儿子).{0,12}kk", joined))
    lele_erjiu = bool(re.search(r"(乐乐|表妹).{0,20}二舅家的闺女|二舅家的闺女.{0,12}(乐乐|你)", joined))
    add("no_kinship_swap", not (kk_erjiu and lele_erjiu), f"kk=二舅之子痕迹={kk_erjiu}；乐乐=二舅闺女={lele_erjiu}")

    # 5 乐乐直球
    love = "我喜欢你" in joined
    add("no_direct_love", not love, "正文出现「我喜欢你」" if love else "无直球喜欢")

    # 6 未成年词
    minor = bool(re.search(r"小学生|幼女|小学女生", joined))
    add("adult_only", not minor, "出现未成年词" if minor else "未写小学生/幼女")

    # 7 修辞
    rhetoric = bool(re.search(r"不是.{0,20}那种", joined))
    add("no_not_that_kind", not rhetoric, "出现「不是…那种」" if rhetoric else "无该套话")

    # 8 穿着/憋尿（软：有则加分）
    skirt = bool(re.search(r"超短裙|短裙|真空", joined))
    pee = bool(re.search(r"憋|小腹", joined))
    add("lele_skirt_or_hold", skirt or pee, f"短裙/真空={skirt} 憋尿痕迹={pee}")

    # 9 摘要复读
    snaps = mem.get("chapter_snapshots") or []
    dump = False
    dump_detail = []
    for i, b in enumerate(bodies):
        snap = next((s for s in snaps if s.get("chapter_id") == b["id"]), None)
        s = (snap or {}).get("summary") or ""
        n = len(s)
        is_dump = n > 400 or (n >= 24 and compact_prefix(s) == compact_prefix(b["text"]))
        dump = dump or is_dump
        dump_detail.append({"ch": i + 1, "chars": n, "dump": is_dump})
    add("summary_not_dump", not dump, json.dumps(dump_detail, ensure_ascii=False))

    # 10 核心愿望未当场兑完
    fulfilled = bool(re.search(r"成为了.{0,6}母狗|当上了.{0,6}母狗", joined))
    add("wish_not_fully_paid", not fulfilled, "当场写完成为母狗" if fulfilled else "未写死兑现")

    passed = sum(1 for c in checks if c["pass"])
    return {
        "score": f"{passed}/{len(checks)}",
        "passed": passed,
        "total": len(checks),
        "checks": checks,
        "chapter_chars": [len(b["text"]) for b in bodies],
        "snapshot_chars": [len((s.get("summary") or "")) for s in snaps],
    }


def main() -> int:
    cli = find_cli()
    print(f"CLI={cli}", flush=True)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    NOVELS.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    folder_name = f"连续锁对照_{stamp}"
    root = NOVELS / folder_name
    root.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    timeline: list[dict] = []

    seeded = seed_title(BOOK_OUTLINE)
    print(f"== create {root} title={seeded}", flush=True)
    run_cli(cli, ["project", "create", str(root), "--title", "未命名小说"], timeout=60)
    pj = root / "project.json"
    disk = json.loads(pj.read_text(encoding="utf-8"))
    disk["book_outline"] = BOOK_OUTLINE
    disk["title"] = seeded
    disk["style"] = (
        "文笔流畅，节奏紧凑，避免流水账。"
        "禁止「不是…是…」「并非…而是…」式否定对照修辞。"
        "乐乐对白含蓄、讨厌但接受，禁止直球表白。"
    )
    pj.write_text(json.dumps(disk, ensure_ascii=False, indent=2), encoding="utf-8")
    timeline.append({"step": "create", "title": seeded, "outline": BOOK_OUTLINE})

    disk = json.loads(pj.read_text(encoding="utf-8"))
    chapters = disk.get("chapters") or []
    if not chapters:
        run_cli(cli, ["chapter", "create", str(root), "第一章", "--summary", ""], timeout=60)
        disk = json.loads(pj.read_text(encoding="utf-8"))
        chapters = disk.get("chapters") or []
    chapter0 = chapters[0]["id"]

    print("== outline_to_chapters", flush=True)
    split = run_cli(
        cli,
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
        raise RuntimeError(f"拆章失败：{split_text[:800]}")
    for row in plan:
        row["must_not"] = compose_must_not(row, BOOK_OUTLINE)
    (OUT_DIR / f"continuity_split_{stamp}.json").write_text(
        json.dumps({"raw": split_text, "plan": plan, "usage": split.get("usage")}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    timeline.append(
        {
            "step": "split",
            "count": len(plan),
            "titles": [c["title"] for c in plan],
            "usage": split.get("usage"),
            "cost_cny": split.get("cost_cny"),
        }
    )

    disk = json.loads(pj.read_text(encoding="utf-8"))
    existing = disk.get("chapters") or []
    applied: list[str] = []
    disk["chapters"][0]["title"] = plan[0]["title"] or "第1章"
    disk["chapters"][0]["summary"] = plan[0]["summary"]
    disk["chapters"][0]["must_do"] = plan[0].get("must_do") or ""
    disk["chapters"][0]["must_not"] = plan[0]["must_not"]
    disk["chapters"][0]["status"] = "pending"
    pj.write_text(json.dumps(disk, ensure_ascii=False, indent=2), encoding="utf-8")
    applied.append(existing[0]["id"])

    for i, ch in enumerate(plan[1:], start=2):
        title = ch["title"] or f"第{i}章"
        created = run_cli(cli, ["chapter", "create", str(root), title, "--summary", ch["summary"]], timeout=60)
        disk = json.loads(pj.read_text(encoding="utf-8"))
        unmatched = [c["id"] for c in disk.get("chapters") or [] if c["id"] not in applied]
        new_id = unmatched[-1] if unmatched else disk["chapters"][-1]["id"]
        for c in disk["chapters"]:
            if c["id"] == new_id:
                c["title"] = title
                c["summary"] = ch["summary"]
                c["must_do"] = ch.get("must_do") or ""
                c["must_not"] = ch["must_not"]
                c["status"] = "pending"
        pj.write_text(json.dumps(disk, ensure_ascii=False, indent=2), encoding="utf-8")
        applied.append(new_id)
        timeline.append({"step": "chapter_create", "id": new_id, "created_keys": list(created.keys())[:8]})

    # 去重 applied 保序
    seen = set()
    uniq = []
    for i in applied:
        if i not in seen:
            seen.add(i)
            uniq.append(i)
    applied = uniq[: len(plan)]

    for cid in applied:
        disk = json.loads(pj.read_text(encoding="utf-8"))
        ch = next(c for c in disk["chapters"] if c["id"] == cid)
        instr = wrap_instruction(ch)
        print(f"== continue {ch.get('title')}", flush=True)
        run_cli(cli, ["chapter", "write", str(root), cid, "--content", ""], timeout=60)
        out = run_cli(
            cli,
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
        timeline.append(
            {
                "step": "continue",
                "title": ch.get("title"),
                "chars": len(text),
                "cost_cny": out.get("cost_cny"),
                "usage": out.get("usage"),
            }
        )
        print(f"   chars={len(text)} cost={out.get('cost_cny')}", flush=True)

        body = read_chapter_body(cli, root, cid)
        print(f"== chapter_summary {ch.get('title')}", flush=True)
        sum_out = run_cli(
            cli,
            [
                "writing",
                "run",
                str(root),
                cid,
                "chapter_summary",
                "--instruction",
                "",
                "--offline",
                "--apply",
                "none",
            ],
            timeout=300,
        )
        stext = sum_out.get("raw_text") or sum_out.get("text") or ""
        timeline.append(
            {
                "step": "chapter_summary",
                "title": ch.get("title"),
                "chars": len(stext),
                "cost_cny": sum_out.get("cost_cny"),
                "dump_like": len(stext) > 400 or compact_prefix(stext) == compact_prefix(body),
            }
        )
        disk = json.loads(pj.read_text(encoding="utf-8"))
        for c in disk["chapters"]:
            if c["id"] == cid:
                c["status"] = "outline_complete"
        pj.write_text(json.dumps(disk, ensure_ascii=False, indent=2), encoding="utf-8")

    analysis = analyze(root, plan, seeded)
    report = {
        "book_outline": BOOK_OUTLINE,
        "root": str(root),
        "cli": str(cli),
        "elapsed_sec": round(time.time() - t0, 1),
        "chapter_plan": plan,
        "timeline": timeline,
        "analysis": analysis,
        "baseline": {
            "old_project": str(NOVELS / "未命名小说17"),
            "old_failures": [
                "晚饭吃两遍",
                "kk与乐乐同写二舅家",
                "乐乐直球我喜欢你",
                "第2章摘要复读全文",
                "上章末段半句切开",
                "书名未命名小说",
            ],
        },
    }
    report_path = OUT_DIR / f"continuity_regression_{stamp}.json"
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"== report {report_path}", flush=True)
    print(json.dumps({"score": analysis["score"], "checks": analysis["checks"], "elapsed_sec": report["elapsed_sec"]}, ensure_ascii=False, indent=2), flush=True)
    return 0 if analysis["passed"] >= analysis["total"] - 2 else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as e:
        print(f"FATAL: {e}", file=sys.stderr)
        raise
