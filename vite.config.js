// 代码路径: kk_novel_ai/vite.config.js
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  build: {
    outDir: "frontend-dist",
  },
  server: {
    port: 5173,
    strictPort: true,
  },
});
