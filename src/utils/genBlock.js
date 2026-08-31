/**
 * 章节生成块：UI 分块数据模型（正文不写 HTML 注释）
 * 代码路径: kk_novel_ai/src/utils/genBlock.js
 */

let _keySeq = 0;

function nextKey() {
  _keySeq += 1;
  return `b-${Date.now().toString(36)}-${_keySeq}`;
}

export function cryptoRandomId() {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  return `g-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

/** @param {string} [text] */
export function createPlainBlock(text = "") {
  return { key: nextKey(), type: "plain", text: String(text ?? "") };
}

/**
 * @param {Partial<{id:string,ts:string,task:string,model:string,chars:number,tokens:number,cost:number,usageSource:string,instruction:string,sources:Array}>} meta
 * @param {string} text
 */
export function createGenBlock(meta = {}, text = "") {
  const body = String(text ?? "");
  return {
    key: nextKey(),
    type: "gen",
    id: meta.id || cryptoRandomId(),
    ts: meta.ts || new Date().toISOString(),
    task: meta.task || "",
    model: meta.model || "",
    chars: meta.chars != null ? Number(meta.chars) : [...body].length,
    tokens: meta.tokens != null && meta.tokens !== "" ? Number(meta.tokens) : null,
    cost: meta.cost != null && meta.cost !== "" ? Number(meta.cost) : null,
    usageSource: meta.usageSource || "",
    instruction: meta.instruction || "",
    /** @type {Array<{kind:string,id?:string,title:string,detail?:string}>} */
    sources: normalizeSources(meta.sources),
    /** 块级蒸馏摘要 */
    digest: meta.digest ? String(meta.digest) : "",
    text: body,
  };
}

/** @param {unknown} raw */
export function normalizeSources(raw) {
  if (!raw) return [];
  const list = Array.isArray(raw)
    ? raw
    : Array.isArray(raw.items)
      ? raw.items
      : [];
  return list
    .filter((x) => x && typeof x === "object")
    .map((x) => ({
      kind: String(x.kind || ""),
      id: String(x.id || ""),
      title: String(x.title || ""),
      detail: String(x.detail || ""),
    }))
    .filter((x) => x.title || x.detail);
}

/** 规范化 sidecar / 内存块 */
export function normalizeBlocks(rawList) {
  if (!Array.isArray(rawList) || !rawList.length) return [createPlainBlock("")];
  return rawList.map((b) => {
    if (!b || typeof b !== "object") return createPlainBlock("");
    if (b.type === "gen") {
      const gen = createGenBlock(
        {
          id: b.id,
          ts: b.ts,
          task: b.task,
          model: b.model,
          chars: b.chars,
          tokens: b.tokens,
          cost: b.cost,
          usageSource: b.usageSource || b.usage || "",
          instruction: b.instruction || "",
          sources: b.sources,
          digest: b.digest || "",
        },
        b.text || ""
      );
      if (b.key) gen.key = b.key;
      return gen;
    }
    return createPlainBlock(b.text || "");
  });
}

/** 块列表 → 落盘正文（无标记） */
export function contentFromBlocks(blocks) {
  if (!Array.isArray(blocks) || !blocks.length) return "";
  const parts = blocks.map((b) => String(b?.text ?? "").replace(/\s+$/g, ""));
  while (parts.length > 1 && !parts[parts.length - 1].trim()) parts.pop();
  return parts.join("\n\n").replace(/^\n+/, "") + (parts.some((p) => p.length) ? "\n" : "");
}

/** 剥旧版 HTML 注释外壳（兼容迁移 / 导出） */
export function stripGenBlocks(content) {
  if (!content) return "";
  return String(content)
    .replace(/<!--\s*kk-gen\b[^>]*-->\s*/gi, "")
    .replace(/<!--\s*\/kk-gen\b[^>]*-->\s*/gi, "");
}

function parseAttrs(attrStr) {
  const out = {};
  const re = /(\w+)="([^"]*)"/g;
  let m;
  while ((m = re.exec(attrStr || ""))) {
    out[m[1]] = m[2];
  }
  return out;
}

/**
 * 从正文解析块：优先 sidecar 数组（与正文长度大致吻合时）；否则迁旧 kk-gen 注释；否则整章 plain
 * format2 分支对象请用 branchModel.parseSidecarToBranchDoc + activePathBlocks（见 projectClient）。
 * @param {string} content
 * @param {unknown} [sidecarBlocks]
 */
export function blocksFromContent(content, sidecarBlocks) {
  const raw = String(content || "");
  const clean = stripGenBlocks(raw);
  const list =
    Array.isArray(sidecarBlocks)
      ? sidecarBlocks
      : sidecarBlocks &&
          typeof sidecarBlocks === "object" &&
          Array.isArray(sidecarBlocks.blocks)
        ? sidecarBlocks.blocks
        : null;
  if (list && list.length) {
    const normalized = normalizeBlocks(list);
    const joined = contentFromBlocks(normalized);
    const fileChars = [...clean.replace(/\s+/g, "")].length;
    const joinChars = [...joined.replace(/\s+/g, "")].length;
    // sidecar 明显丢了正文时，以文件为准，避免编辑器只剩短块 + 大片空
    if (fileChars > joinChars + 80) {
      if (/<!--\s*kk-gen\b/i.test(raw)) {
        return parseMarkedToBlocks(raw);
      }
      return [createPlainBlock(clean)];
    }
    return normalized;
  }
  if (/<!--\s*kk-gen\b/i.test(raw)) {
    return parseMarkedToBlocks(raw);
  }
  return [createPlainBlock(raw)];
}

function parseMarkedToBlocks(raw) {
  const blocks = [];
  const openRe = /<!--\s*kk-gen\b([^>]*)-->/gi;
  let last = 0;
  let om;
  while ((om = openRe.exec(raw))) {
    const before = raw.slice(last, om.index);
    if (before.trim() || blocks.length === 0) {
      if (before.length) blocks.push(createPlainBlock(before.replace(/^\n+|\n+$/g, "")));
    }
    const attrs = parseAttrs(om[1]);
    const afterOpen = om.index + om[0].length;
    const closeRe = /<!--\s*\/kk-gen\b[^>]*-->/gi;
    closeRe.lastIndex = afterOpen;
    const cm = closeRe.exec(raw);
    if (!cm) {
      const rest = raw.slice(afterOpen).replace(/^\n+/, "");
      blocks.push(
        createGenBlock(
          {
            id: attrs.id,
            ts: attrs.ts,
            task: attrs.task,
            model: attrs.model,
            chars: attrs.chars ? Number(attrs.chars) : undefined,
            tokens: attrs.tokens ? Number(attrs.tokens) : undefined,
            cost: attrs.cost ? Number(attrs.cost) : undefined,
            usageSource: attrs.usage || "",
          },
          rest.trim()
        )
      );
      last = raw.length;
      break;
    }
    const body = raw.slice(afterOpen, cm.index).replace(/^\n+|\n+$/g, "");
    blocks.push(
      createGenBlock(
        {
          id: attrs.id,
          ts: attrs.ts,
          task: attrs.task,
          model: attrs.model,
          chars: attrs.chars ? Number(attrs.chars) : undefined,
          tokens: attrs.tokens ? Number(attrs.tokens) : undefined,
          cost: attrs.cost ? Number(attrs.cost) : undefined,
          usageSource: attrs.usage || "",
        },
        body
      )
    );
    last = cm.index + cm[0].length;
    openRe.lastIndex = last;
  }
  if (last < raw.length) {
    const tail = raw.slice(last).replace(/^\n+/, "");
    if (tail.trim()) blocks.push(createPlainBlock(tail));
  }
  if (!blocks.length) return [createPlainBlock(stripGenBlocks(raw))];
  return blocks;
}

/** 落盘 sidecar 用（去掉临时 key） */
export function blocksForPersist(blocks) {
  return (blocks || []).map((b) => {
    if (b.type === "gen") {
      return {
        type: "gen",
        key: b.key || "",
        id: b.id,
        ts: b.ts,
        task: b.task || "",
        model: b.model || "",
        chars: b.chars,
        tokens: b.tokens,
        cost: b.cost,
        usageSource: b.usageSource || "",
        instruction: b.instruction || "",
        sources: normalizeSources(b.sources),
        digest: b.digest || "",
        text: b.text || "",
      };
    }
    return { type: "plain", key: b.key || "", text: b.text || "" };
  });
}

/** 块下方信息条文案 */
export function formatBlockMeta(block) {
  if (!block || block.type !== "gen") return "";
  const parts = [];
  if (block.task) parts.push(block.task);
  if (block.model) parts.push(block.model);
  const chars = block.chars != null ? block.chars : [...String(block.text || "")].length;
  parts.push(`${chars} 字`);
  if (block.tokens != null && Number(block.tokens) > 0) {
    const src = block.usageSource === "api" ? "api" : block.usageSource ? "估" : "";
    parts.push(`tokens ${block.tokens}${src ? ` (${src})` : ""}`);
  }
  if (block.cost != null && Number(block.cost) > 0) {
    parts.push(`¥${Number(block.cost).toFixed(4)}`);
  }
  if (block.ts) {
    try {
      const d = new Date(block.ts);
      if (!Number.isNaN(d.getTime())) {
        parts.push(
          `${d.getMonth() + 1}/${d.getDate()} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`
        );
      }
    } catch {
      /* ignore */
    }
  }
  if (block.id) parts.push(`id ${String(block.id).slice(0, 8)}`);
  return parts.join(" · ");
}

const SOURCE_KIND_LABEL = {
  instruction: "指令",
  outline: "章纲",
  pov: "POV",
  arc: "故事弧",
  must_do: "必达",
  beat: "节拍",
  lore: "设定",
};

/**
 * 生成块「设定来源」一行摘要
 * @param {{type?:string,instruction?:string,sources?:Array<{kind:string,title:string,detail?:string}>}} block
 */
export function formatBlockSources(block) {
  if (!block || block.type !== "gen") return "";
  const items = normalizeSources(block.sources);
  const parts = [];
  const byKind = (k) => items.filter((x) => x.kind === k);

  const instr = byKind("instruction")[0];
  if (instr?.detail) {
    parts.push(`指令「${instr.detail}」`);
  } else if ((block.instruction || "").trim()) {
    const t = String(block.instruction).trim();
    parts.push(`指令「${t.length > 80 ? `${t.slice(0, 80)}…` : t}」`);
  }

  const outline = byKind("outline")[0];
  if (outline?.title) {
    parts.push(`章纲「${outline.title}」`);
  }

  const pov = byKind("pov")[0];
  if (pov?.title) parts.push(`POV ${pov.title}`);

  const arcs = byKind("arc").map((x) => x.title).filter(Boolean);
  if (arcs.length) parts.push(`弧 ${arcs.join("、")}`);

  const must = byKind("must_do")[0];
  if (must?.detail) parts.push(`必达「${must.detail}」`);

  const lore = byKind("lore").map((x) => x.title).filter(Boolean);
  if (lore.length) {
    const shown = lore.slice(0, 6);
    parts.push(`设定 ${shown.join("、")}${lore.length > 6 ? ` 等${lore.length}条` : ""}`);
  }

  const beats = byKind("beat").map((x) => x.title).filter(Boolean);
  if (beats.length && parts.length < 5) {
    parts.push(`节拍 ${beats.slice(0, 3).join("、")}`);
  }

  if (!parts.length && items.length) {
    return items
      .slice(0, 8)
      .map((x) => {
        const label = SOURCE_KIND_LABEL[x.kind] || x.kind || "来源";
        return x.detail ? `${label}「${x.title || x.detail}」` : `${label} ${x.title}`;
      })
      .join(" · ");
  }
  return parts.join(" · ");
}

/**
 * 按空行拆自然段（生成块内逐段挂来源用）
 * @param {string} text
 * @returns {string[]}
 */
export function splitNaturalParagraphs(text) {
  const raw = String(text ?? "");
  if (!raw) return [""];
  const parts = raw.split(/\n(?:[ \t]*\n)+/);
  return parts.length ? parts : [""];
}

/** @param {string[]} paras */
export function joinNaturalParagraphs(paras) {
  if (!Array.isArray(paras) || !paras.length) return "";
  return paras.map((p) => String(p ?? "").replace(/\s+$/g, "")).join("\n\n");
}

/**
 * 自然段在全文中的起止（含段间 \n\n）
 * @param {string} text
 * @returns {Array<{start:number,end:number,text:string}>}
 */
export function naturalParagraphRanges(text) {
  const paras = splitNaturalParagraphs(text);
  const ranges = [];
  let cursor = 0;
  const raw = String(text ?? "");
  for (let i = 0; i < paras.length; i++) {
    const p = paras[i];
    let start = raw.indexOf(p, cursor);
    if (start < 0) start = cursor;
    const end = start + p.length;
    ranges.push({ start, end, text: p });
    cursor = end;
    while (cursor < raw.length && (raw[cursor] === "\n" || raw[cursor] === " " || raw[cursor] === "\t")) {
      cursor += 1;
    }
  }
  return ranges;
}

function loreTitleVariants(title) {
  const t = String(title || "").trim();
  if (!t) return [];
  const bare = t.replace(/^\[[^\]]*\]\s*/, "").trim();
  return [...new Set([t, bare].filter(Boolean))];
}

/**
 * 为本段挑选更相关的来源（设定按段内点名优先）
 * @param {string} paraText
 * @param {{type?:string,instruction?:string,sources?:unknown}} block
 */
export function pickSourcesForParagraph(paraText, block) {
  const items = normalizeSources(block && block.sources);
  const sharedKinds = new Set(["instruction", "outline", "pov", "arc", "must_do", "beat"]);
  const shared = items.filter((i) => sharedKinds.has(i.kind));
  const lore = items.filter((i) => i.kind === "lore");
  const text = String(paraText || "");
  const matched = lore.filter((l) =>
    loreTitleVariants(l.title).some((v) => v.length >= 1 && text.includes(v))
  );
  return {
    shared,
    lore: matched.length ? matched : lore,
    loreMatched: matched.length > 0,
    loreTotal: lore.length,
  };
}

/**
 * 自然段来源一行
 * @param {string} paraText
 * @param {{type?:string,instruction?:string,sources?:unknown}} block
 */
export function formatParagraphSources(paraText, block) {
  if (!block || block.type !== "gen") return "";
  const picked = pickSourcesForParagraph(paraText, block);
  const fake = {
    type: "gen",
    instruction: block.instruction || "",
    sources: [
      ...picked.shared,
      ...picked.lore.map((l) => ({ ...l, kind: "lore" })),
    ],
  };
  let line = formatBlockSources(fake);
  if (!line) return "";
  if (picked.loreTotal && !picked.loreMatched && picked.lore.length) {
    line = line.replace(/设定 /, "注入设定 ");
  } else if (picked.loreMatched) {
    line = line.replace(/设定 /, "相关设定 ");
  }
  return line;
}

/**
 * 自然段总结横条：短标题
 * @param {string} paraText
 * @param {number} [maxLen]
 */
export function paragraphSummaryLabel(paraText, maxLen = 36) {
  const t = String(paraText || "")
    .replace(/\s+/g, " ")
    .trim();
  if (!t) return "（空段）";
  const n = Math.max(8, maxLen || 36);
  return t.length > n ? `${t.slice(0, n)}…` : t;
}

function bareSourceTitle(title) {
  return String(title || "")
    .replace(/^\[[^\]]*\]\s*/, "")
    .trim();
}

/**
 * 自然段总结横条：标签 chips
 * @param {string} paraText
 * @param {{type?:string,instruction?:string,sources?:unknown}} block
 * @returns {Array<{kind:string,label:string}>}
 */
export function paragraphSummaryTags(paraText, block) {
  if (!block || block.type !== "gen") return [];
  const picked = pickSourcesForParagraph(paraText, block);
  /** @type {Array<{kind:string,label:string}>} */
  const tags = [];
  const push = (kind, label) => {
    const s = String(label || "").trim();
    if (!s) return;
    if (tags.some((t) => t.kind === kind && t.label === s)) return;
    tags.push({ kind, label: s.length > 18 ? `${s.slice(0, 18)}…` : s });
  };

  const instr = picked.shared.find((i) => i.kind === "instruction");
  if (instr?.detail) push("instruction", instr.detail);
  else if ((block.instruction || "").trim()) push("instruction", block.instruction);

  const must = picked.shared.find((i) => i.kind === "must_do");
  if (must?.detail) push("must_do", must.detail);

  const pov = picked.shared.find((i) => i.kind === "pov");
  if (pov?.title) push("pov", bareSourceTitle(pov.title));

  for (const a of picked.shared.filter((i) => i.kind === "arc").slice(0, 2)) {
    push("arc", bareSourceTitle(a.title));
  }

  const loreList = picked.loreMatched ? picked.lore : picked.lore.slice(0, 4);
  for (const l of loreList.slice(0, 5)) {
    push("lore", bareSourceTitle(l.title));
  }

  return tags;
}

/**
 * 侧栏段落索引文案（优先指令，否则正文开头）
 * @param {{type?:string,instruction?:string,text?:string}} block
 * @param {number} [index]
 */
export function blockTocLabel(block, index = 0) {
  if (!block) return `段落 ${(index || 0) + 1}`;
  const instr = String(block.instruction || "").trim();
  if (instr) return paragraphSummaryLabel(instr, 28);
  const fromText = paragraphSummaryLabel(block.text || "", 28);
  if (fromText && fromText !== "（空段）") return fromText;
  return `段落 ${(index || 0) + 1}`;
}

/**
 * 章节下生成块目录
 * @param {Array<{type?:string,key?:string,instruction?:string,text?:string}>} blocks
 * @returns {Array<{key:string,label:string,index:number}>}
 */
export function genBlocksToc(blocks) {
  const list = Array.isArray(blocks) ? blocks : [];
  const out = [];
  let genIndex = 0;
  list.forEach((b, i) => {
    if (!b || b.type !== "gen") return;
    out.push({
      key: b.key || `gen-${i}`,
      label: blockTocLabel(b, genIndex),
      index: i,
      genIndex,
    });
    genIndex += 1;
  });
  return out;
}

/**
 * @deprecated 已改为 UI 块；保留空实现避免旧引用炸
 * 请用 createGenBlock + 编辑器分块
 */
export function wrapGenBlock(meta, body) {
  return String(body || "").trim();
}
