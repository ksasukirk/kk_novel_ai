/**
 * 大纲页导图：本地拆全书大纲 / 拆章纲要点 / 按结构或缓存选树
 * 代码路径: kk_novel_ai/src/utils/outlineMindTree.js
 */

/**
 * 已拆出实质章纲（不是默认空第一章）
 * @param {{ chapters?: Array, volumes?: Array } | null} project
 */
export function hasStructuredOutline(project) {
  const chapters = (project && project.chapters) || [];
  const volumes = (project && project.volumes) || [];
  if (chapters.length > 1) return true;
  if (
    chapters.some(
      (c) =>
        String((c && c.summary) || "").trim() ||
        (Array.isArray(c && c.beats) && c.beats.length)
    )
  ) {
    return true;
  }
  if (
    volumes.some(
      (v) =>
        String((v && v.arc_goal) || "").trim() ||
        String((v && v.arc_summary) || "").trim()
    )
  ) {
    return true;
  }
  return false;
}

/**
 * 把章纲拆成要点（冲突/推进/钩子、换行、分号）
 * @param {string} summary
 * @returns {string[]}
 */
export function parseChapterPoints(summary) {
  const raw = String(summary || "").trim();
  if (!raw) return [];
  const labeled = [];
  const re = /(冲突|推进|钩子|必达|场景)\s*[:：]\s*([^\n；;]+)/g;
  let m;
  while ((m = re.exec(raw))) {
    const text = String(m[2] || "").trim();
    if (text) labeled.push(`${m[1]}：${text}`);
  }
  if (labeled.length) return labeled.slice(0, 8);

  const parts = raw
    .split(/\n+|；|;|。/)
    .map((s) => s.trim())
    .filter((s) => s.length >= 2);
  if (parts.length >= 2) return parts.slice(0, 8);
  return raw.length > 36 ? [raw.slice(0, 36)] : [raw];
}

function trimLabel(s, n = 24) {
  const t = String(s || "").replace(/\s+/g, " ").trim();
  if (t.length <= n) return t;
  return `${t.slice(0, n - 1)}…`;
}

function node(id, label, kind, meta, children = []) {
  return { id, label: trimLabel(label) || "节点", kind, meta: String(meta || ""), children };
}

/**
 * 本地拆全书大纲文本
 * @param {string} text
 * @param {string} [title]
 * @returns {{ tree: object, thin: boolean }}
 */
export function parseBookOutlineLocal(text, title = "作品") {
  const raw = String(text || "").trim();
  const root = node("root", title || "作品", "root", "情节导图", []);
  if (!raw) return { tree: root, thin: true };

  const chapterChunks = splitByChapterHeadings(raw);
  if (chapterChunks.length >= 2) {
    root.children = chapterChunks.map((c, i) =>
      node(
        `local:ch:${i + 1}`,
        c.title,
        "chapter",
        c.body,
        parseChapterPoints(c.body).map((p, j) =>
          node(`local:ch:${i + 1}:p:${j}`, p, "point", "")
        )
      )
    );
    return { tree: root, thin: false };
  }

  const mdHeads = splitByMarkdownHeads(raw);
  if (mdHeads.length >= 2) {
    root.children = mdHeads.map((c, i) =>
      node(`local:h:${i + 1}`, c.title, "plot", c.body, [])
    );
    return { tree: root, thin: false };
  }

  const numbered = splitByNumberedList(raw);
  if (numbered.length >= 3) {
    root.children = numbered.map((c, i) =>
      node(`local:n:${i + 1}`, c.title, "point", c.body, [])
    );
    return { tree: root, thin: false };
  }

  const paras = raw
    .split(/\n\s*\n/)
    .map((s) => s.trim())
    .filter((s) => s.length >= 8);
  if (paras.length >= 3) {
    root.children = paras.slice(0, 16).map((p, i) =>
      node(`local:p:${i + 1}`, p, "point", p, [])
    );
    return { tree: root, thin: false };
  }

  root.children = [
    node("local:hint", "点「整理成导图」用 AI 拆树", "point", raw.slice(0, 80)),
  ];
  return { tree: root, thin: true };
}

function splitByChapterHeadings(raw) {
  const re = /(?=第\s*[一二三四五六七八九十百千0-9]+\s*章)/g;
  const parts = raw.split(re).map((s) => s.trim()).filter(Boolean);
  if (parts.length < 2) return [];
  return parts.map((block, i) => {
    const nl = block.indexOf("\n");
    const title = (nl >= 0 ? block.slice(0, nl) : block).trim() || `第${i + 1}章`;
    const body = (nl >= 0 ? block.slice(nl + 1) : "").trim();
    return { title, body };
  });
}

function splitByMarkdownHeads(raw) {
  const re = /^(#{1,3})\s+(.+)$/gm;
  const hits = [];
  let m;
  while ((m = re.exec(raw))) {
    hits.push({ index: m.index, title: m[2].trim(), end: m.index + m[0].length });
  }
  if (hits.length < 2) return [];
  return hits.map((h, i) => {
    const from = h.end;
    const to = i + 1 < hits.length ? hits[i + 1].index : raw.length;
    return { title: h.title, body: raw.slice(from, to).trim() };
  });
}

function splitByNumberedList(raw) {
  const re = /(?:^|\n)\s*(?:\d+[\.、]|[一二三四五六七八九十]+[、.])\s*/g;
  const parts = raw.split(re).map((s) => s.trim()).filter(Boolean);
  if (parts.length < 3) return [];
  return parts.map((body, i) => {
    const line = body.split("\n")[0] || body;
    return { title: line, body };
  });
}

function toMindNode(n, fallbackId, depth = 0) {
  if (!n || typeof n !== "object" || depth > 5) return null;
  const id = String(n.id || fallbackId || "n");
  const children = Array.isArray(n.children)
    ? n.children
        .map((c, i) => toMindNode(c, `${id}:${i}`, depth + 1))
        .filter(Boolean)
        .slice(0, 24)
    : [];
  return {
    id,
    label: trimLabel(n.label || n.title || "节点"),
    kind: String(n.kind || "point"),
    meta: String(n.summary || n.meta || ""),
    children,
  };
}

/**
 * 把落盘 outline_mindmap 转成 MindMapBoard 树
 * @param {object | null} saved
 * @param {string} [title]
 */
export function mindNodeFromSaved(saved, title = "作品") {
  if (!saved || typeof saved !== "object") return null;
  const root = saved.root || saved;
  const mapped = toMindNode(root, "root");
  if (!mapped) return null;
  if (!mapped.label) mapped.label = title || "作品";
  mapped.kind = mapped.kind || "root";
  return mapped;
}

/**
 * 解析模型输出的导图 JSON
 * @param {string} text
 * @returns {{ reason: string, root: object | null }}
 */
export function parseOutlineMindmap(text) {
  const raw = String(text || "").trim();
  if (!raw) return { reason: "", root: null };
  let body = raw;
  const fence = raw.match(/```(?:json)?\s*([\s\S]*?)```/i);
  if (fence) body = fence[1].trim();
  const start = body.indexOf("{");
  const end = body.lastIndexOf("}");
  if (start < 0 || end <= start) return { reason: "", root: null };
  let data;
  try {
    data = JSON.parse(body.slice(start, end + 1));
  } catch {
    return { reason: "", root: null };
  }
  const reason = String((data && data.reason) || "").trim();
  const root = toMindNode((data && data.root) || data, "root");
  if (!root || !(root.children || []).length) {
    return { reason, root: null };
  }
  return { reason, root };
}

/**
 * 大纲页选树：结构 / 缓存情节树 / 本地拆树
 * @param {{
 *   project: object,
 *   snapshots?: Record<string, string>,
 *   prefer?: "auto" | "structure" | "plot",
 *   buildStructureTree: (opts: object) => object
 * }} opts
 */
export function buildOutlinePageTree({
  project,
  snapshots = {},
  prefer = "auto",
  buildStructureTree,
}) {
  const title = (project && project.title) || "作品";
  const structured = hasStructuredOutline(project);
  const saved = mindNodeFromSaved(project && project.outline_mindmap, title);
  const book = String((project && project.book_outline) || "").trim();
  const local = parseBookOutlineLocal(book, title);

  if (prefer === "structure" && typeof buildStructureTree === "function") {
    return { tree: buildStructureTree({ snapshots }), source: "structure", thin: false };
  }
  if (prefer === "plot") {
    if (saved) return { tree: saved, source: "saved", thin: false };
    return { tree: local.tree, source: "local", thin: local.thin };
  }

  if (structured && typeof buildStructureTree === "function") {
    return { tree: buildStructureTree({ snapshots }), source: "structure", thin: false };
  }
  if (saved) return { tree: saved, source: "saved", thin: false };
  return { tree: local.tree, source: "local", thin: local.thin };
}
