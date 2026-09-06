/*
 * Kk Novel Ai Vue 应用入口
 * 代码路径: kk_novel_ai/src/main.js
 */
import { createApp } from "vue";
import App from "./App.vue";
import { i18n } from "./i18n/index.js";
import "./style.css";

createApp(App).use(i18n).mount("#app");
