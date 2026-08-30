// 代码路径: build-frontend.mjs
import { execSync } from "node:child_process";
import path from "node:path";
import url from "node:url";

const __filename = url.fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

try {
  execSync("npm run build", {
    cwd: __dirname,
    stdio: "inherit",
  });
} catch (error) {
  console.error("[frontend] 执行 vite build 失败:", error?.message || String(error));
  process.exit(1);
}
