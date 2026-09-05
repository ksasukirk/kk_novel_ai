/**
 * 解析模型输出的近似 JSON（围栏、漏逗号、拖尾逗号、截断对象）
 * 代码路径: kk_novel_ai/src/utils/llmJson.js
 */

function stripFence(text) {
  let s = String(text || "").trim();
  if (s.charCodeAt(0) === 0xfeff) s = s.slice(1);
  const fence = s.match(/```(?:json)?\s*([\s\S]*?)```/i);
  if (fence) s = fence[1].trim();
  return s;
}

function skipWs(s, i) {
  while (i < s.length && /\s/.test(s[i])) i += 1;
  return i;
}

function scanString(s, i) {
  if (s[i] !== '"') return -1;
  i += 1;
  while (i < s.length) {
    const c = s[i];
    if (c === "\\") {
      i += 2;
      continue;
    }
    if (c === '"') return i + 1;
    i += 1;
  }
  return -1;
}

function extractBalanced(s) {
  const objStart = s.indexOf("{");
  const arrStart = s.indexOf("[");
  let start = -1;
  if (arrStart >= 0 && (objStart < 0 || arrStart < objStart)) start = arrStart;
  else if (objStart >= 0) start = objStart;
  else return "";
  let depth = 0;
  let inStr = false;
  let esc = false;
  for (let i = start; i < s.length; i += 1) {
    const c = s[i];
    if (inStr) {
      if (esc) {
        esc = false;
        continue;
      }
      if (c === "\\") {
        esc = true;
        continue;
      }
      if (c === '"') inStr = false;
      continue;
    }
    if (c === '"') {
      inStr = true;
      continue;
    }
    if (c === "{" || c === "[") depth += 1;
    else if (c === "}" || c === "]") {
      depth -= 1;
      if (depth === 0) return s.slice(start, i + 1);
    }
  }
  return s.slice(start);
}

function stripCommentsOutsideStrings(s) {
  let out = "";
  let inStr = false;
  let esc = false;
  for (let i = 0; i < s.length; i += 1) {
    const c = s[i];
    if (inStr) {
      out += c;
      if (esc) {
        esc = false;
        continue;
      }
      if (c === "\\") {
        esc = true;
        continue;
      }
      if (c === '"') inStr = false;
      continue;
    }
    if (c === '"') {
      inStr = true;
      out += c;
      continue;
    }
    if (c === "/" && s[i + 1] === "/") {
      while (i < s.length && s[i] !== "\n") i += 1;
      out += "\n";
      continue;
    }
    if (c === "/" && s[i + 1] === "*") {
      i += 2;
      while (i < s.length && !(s[i] === "*" && s[i + 1] === "/")) i += 1;
      i += 1;
      continue;
    }
    out += c;
  }
  return out;
}

function dropTrailingCommas(s) {
  let out = "";
  let inStr = false;
  let esc = false;
  for (let i = 0; i < s.length; i += 1) {
    const c = s[i];
    if (inStr) {
      out += c;
      if (esc) {
        esc = false;
        continue;
      }
      if (c === "\\") {
        esc = true;
        continue;
      }
      if (c === '"') inStr = false;
      continue;
    }
    if (c === '"') {
      inStr = true;
      out += c;
      continue;
    }
    if (c === ",") {
      let j = i + 1;
      while (j < s.length && /\s/.test(s[j])) j += 1;
      if (s[j] === "}" || s[j] === "]") continue;
    }
    out += c;
  }
  return out;
}

function insertMissingCommas(s) {
  let out = "";
  let i = 0;
  const n = s.length;

  const copyWs = () => {
    const start = i;
    i = skipWs(s, i);
    out += s.slice(start, i);
  };

  const copyRange = (from, to) => {
    out += s.slice(from, to);
    i = to;
  };

  const scanValue = () => {
    copyWs();
    if (i >= n) return false;
    const c = s[i];
    if (c === '"') {
      const end = scanString(s, i);
      if (end < 0) {
        copyRange(i, n);
        return false;
      }
      copyRange(i, end);
      return true;
    }
    if (c === "{") {
      copyRange(i, i + 1);
      while (i < n) {
        copyWs();
        if (s[i] === "}") {
          copyRange(i, i + 1);
          return true;
        }
        if (!scanValue()) return false;
        copyWs();
        if (s[i] !== ":") {
          out += ":";
        } else {
          copyRange(i, i + 1);
        }
        if (!scanValue()) return false;
        copyWs();
        if (s[i] === ",") {
          copyRange(i, i + 1);
          copyWs();
          if (s[i] === "}") {
            copyRange(i, i + 1);
            return true;
          }
          continue;
        }
        if (s[i] === "}") {
          copyRange(i, i + 1);
          return true;
        }
        if (s[i] === '"' || s[i] === "{" || s[i] === "[") {
          out += ",";
          continue;
        }
        copyRange(i, i + 1);
      }
      return false;
    }
    if (c === "[") {
      copyRange(i, i + 1);
      while (i < n) {
        copyWs();
        if (s[i] === "]") {
          copyRange(i, i + 1);
          return true;
        }
        if (!scanValue()) return false;
        copyWs();
        if (s[i] === ",") {
          copyRange(i, i + 1);
          copyWs();
          if (s[i] === "]") {
            copyRange(i, i + 1);
            return true;
          }
          continue;
        }
        if (s[i] === "]") {
          copyRange(i, i + 1);
          return true;
        }
        if (
          s[i] === "{" ||
          s[i] === "[" ||
          s[i] === '"' ||
          s[i] === "-" ||
          (s[i] >= "0" && s[i] <= "9") ||
          s.startsWith("true", i) ||
          s.startsWith("false", i) ||
          s.startsWith("null", i)
        ) {
          out += ",";
          continue;
        }
        copyRange(i, i + 1);
      }
      return false;
    }
    if (c === "-" || (c >= "0" && c <= "9")) {
      const start = i;
      i += 1;
      while (i < n && /[0-9.eE+-]/.test(s[i])) i += 1;
      out += s.slice(start, i);
      return true;
    }
    if (s.startsWith("true", i)) {
      copyRange(i, i + 4);
      return true;
    }
    if (s.startsWith("false", i)) {
      copyRange(i, i + 5);
      return true;
    }
    if (s.startsWith("null", i)) {
      copyRange(i, i + 4);
      return true;
    }
    copyRange(i, i + 1);
    return true;
  };

  scanValue();
  if (i < n) out += s.slice(i);
  return out;
}

function tryParse(s) {
  return JSON.parse(s);
}

function findBalancedObject(s, start) {
  if (s[start] !== "{") return -1;
  let depth = 0;
  let inStr = false;
  let esc = false;
  for (let i = start; i < s.length; i += 1) {
    const c = s[i];
    if (inStr) {
      if (esc) {
        esc = false;
        continue;
      }
      if (c === "\\") {
        esc = true;
        continue;
      }
      if (c === '"') inStr = false;
      continue;
    }
    if (c === '"') {
      inStr = true;
      continue;
    }
    if (c === "{") depth += 1;
    else if (c === "}") {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function salvageShots(s) {
  const shots = [];
  for (let i = 0; i < s.length; i += 1) {
    if (s[i] !== "{") continue;
    const end = findBalancedObject(s, i);
    if (end < 0) continue;
    const slice = s.slice(i, end + 1);
    if (!/"visual"\s*:/.test(slice) && !/"seq"\s*:/.test(slice)) continue;
    try {
      const obj = tryParse(dropTrailingCommas(slice));
      if (obj && typeof obj === "object" && !Array.isArray(obj)) {
        if (Array.isArray(obj.shots)) continue;
        shots.push(obj);
        i = end;
      }
    } catch {
      /* skip */
    }
  }
  return shots;
}

function candidatesFrom(text) {
  const stripped = stripFence(text);
  const list = [];
  const push = (s) => {
    if (s && !list.includes(s)) list.push(s);
  };
  push(stripped);
  const balanced = extractBalanced(stripped);
  push(balanced);
  return list.map((s) => stripCommentsOutsideStrings(s));
}

/**
 * @param {string} text
 * @returns {any}
 */
export function parseLlmJson(text) {
  const raw = String(text || "").trim();
  if (!raw) throw new Error("模型没有返回内容");
  let lastErr = null;
  const bodies = candidatesFrom(raw);
  for (const body of bodies) {
    const variants = [body, dropTrailingCommas(body), insertMissingCommas(dropTrailingCommas(body))];
    for (const v of variants) {
      try {
        return tryParse(v);
      } catch (e) {
        lastErr = e;
      }
    }
  }
  for (const body of bodies) {
    const shots = salvageShots(insertMissingCommas(dropTrailingCommas(body)));
    if (shots.length) return { shots };
  }
  const hint = lastErr && lastErr.message ? lastErr.message : "未知错误";
  throw new Error(
    `模型返回的 JSON 无法解析（${hint}）。请再点一次生成；若仍失败，可把本章节拍缩短后再试。`
  );
}
