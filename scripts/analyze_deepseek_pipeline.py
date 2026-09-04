#!/usr/bin/env python3
# 代码路径: kk_novel_ai/scripts/analyze_deepseek_pipeline.py
import json
from pathlib import Path

proj = Path(r"D:\KKFiles\KKProjects\Kinit\kk_novel_ai\dist\novels\DeepSeekCacheTest_0904_0048")
pj = json.loads((proj / "project.json").read_text(encoding="utf-8"))
print("=== 章纲 ===")
for i, c in enumerate(pj["chapters"], 1):
    print(f"{i}. {c.get('title')}")
    print(f"   summary: {c.get('summary')}")
    print(f"   must_do: {c.get('must_do')}")

print("=== 正文文件 ===")
total = 0
for f in sorted((proj / "chapters").glob("*.md")):
    body = f.read_text(encoding="utf-8")
    n = len(body)
    nw = len("".join(body.split()))
    total += n
    print(f"{f.name}: raw={n} no_ws={nw}")
print("total_raw", total)

items = []
app = Path.home() / "AppData" / "Roaming" / "kk_novel_ai" / "gen_log.jsonl"
for line in app.read_text(encoding="utf-8", errors="replace").splitlines():
    if "DeepSeekCacheTest_0904_0048" not in line:
        continue
    items.append(json.loads(line))

hit = sum(int((e.get("usage") or {}).get("prompt_cache_hit_tokens") or 0) for e in items)
miss = sum(int((e.get("usage") or {}).get("prompt_cache_miss_tokens") or 0) for e in items)
prompt = sum(int((e.get("usage") or {}).get("prompt_tokens") or 0) for e in items)
comp = sum(int((e.get("usage") or {}).get("completion_tokens") or 0) for e in items)
cost = sum(float(e.get("cost_cny") or 0) for e in items)
chars = sum(int(e.get("chars_final") or 0) for e in items if e.get("task") == "continue")

cost_no_cache = (prompt / 1e6) * 1.5 + (comp / 1e6) * 4.5
saved = cost_no_cache - cost

print("=== 性价比 ===")
print(
    json.dumps(
        {
            "model": "deepseek-v4-flash",
            "tier": "idle (北京约 00:48)",
            "calls": len(items),
            "continue_chars": chars,
            "body_file_chars": total,
            "prompt_tokens": prompt,
            "completion_tokens": comp,
            "cache_hit": hit,
            "cache_miss": miss,
            "hit_rate_pct": round(hit / (hit + miss) * 100, 2) if hit + miss else None,
            "cost_cny_actual": round(cost, 6),
            "cost_if_no_cache": round(cost_no_cache, 6),
            "cost_saved_by_cache": round(saved, 6),
            "cache_save_pct_vs_full_miss": round(saved / cost_no_cache * 100, 2) if cost_no_cache else None,
            "yuan_per_1k_chars_continue": round(cost / (chars / 1000), 6) if chars else None,
            "chars_per_yuan": round(chars / cost, 1) if cost else None,
        },
        ensure_ascii=False,
        indent=2,
    )
)

print("=== 分次调用 ===")
for e in items:
    u = e.get("usage") or {}
    h = int(u.get("prompt_cache_hit_tokens") or 0)
    m = int(u.get("prompt_cache_miss_tokens") or 0)
    cf = int(e.get("chars_final") or 0)
    cc = float(e.get("cost_cny") or 0)
    print(
        json.dumps(
            {
                "task": e.get("task"),
                "chars": cf,
                "cost": round(cc, 6),
                "prompt": u.get("prompt_tokens"),
                "comp": u.get("completion_tokens"),
                "hit": h,
                "miss": m,
                "hit_rate": round(h / (h + m) * 100, 1) if h + m else 0,
                "yuan_per_1k": round(cc / (cf / 1000), 4) if cf else None,
            },
            ensure_ascii=False,
        )
    )
