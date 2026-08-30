/**
 * 初始化 Android 工程（缺则 tauri android init）并打补丁
 * 代码路径: kk_novel_ai/scripts/android-setup.mjs
 *
 * 前置: 可由 build_android.py 自动引导 JDK / ANDROID_HOME / NDK
 * 用法: node scripts/android-setup.mjs
 */
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const genAndroid = path.join(root, "src-tauri", "gen", "android");
const iconPng = path.join(root, "src-tauri", "icons", "icon.png");
const gradleKts = path.join(genAndroid, "app", "build.gradle.kts");
const manifestPath = path.join(
  genAndroid,
  "app",
  "src",
  "main",
  "AndroidManifest.xml",
);

function run(cmd, args) {
  console.log(`[android-setup] ${cmd} ${args.join(" ")}`);
  const r = spawnSync(cmd, args, {
    cwd: root,
    stdio: "inherit",
    shell: true,
    env: { ...process.env, CI: "true" },
  });
  if (r.status !== 0) {
    process.exit(r.status ?? 1);
  }
}

function isAndroidProjectReady() {
  return fs.existsSync(gradleKts);
}

function patchResizeableActivity(xmlPath) {
  let xml = fs.readFileSync(xmlPath, "utf8");
  if (!xml.includes("android:resizeableActivity=")) {
    xml = xml.replace(
      /(<activity\b[^>]*android:name="\.MainActivity"[^>]*)>/,
      '$1 android:resizeableActivity="true">',
    );
    if (!xml.includes("android:resizeableActivity=")) {
      xml = xml.replace(
        /<activity\b([^>]*)>/,
        '<activity$1 android:resizeableActivity="true">',
      );
    }
  }
  if (!xml.includes("android:roundIcon=")) {
    xml = xml.replace(
      /android:icon="@mipmap\/ic_launcher"/,
      'android:icon="@mipmap/ic_launcher"\n        android:roundIcon="@mipmap/ic_launcher_round"',
    );
  }
  fs.writeFileSync(xmlPath, xml, "utf8");
}

/** 同步启动图标 */
function syncLauncherIcons() {
  if (!fs.existsSync(iconPng)) {
    console.warn(`[android-setup] 缺少图标源: ${iconPng}，跳过图标同步`);
    return;
  }
  if (!fs.existsSync(path.join(genAndroid, "app"))) {
    console.warn("[android-setup] gen/android 未就绪，跳过图标同步");
    return;
  }
  console.log("[android-setup] 同步图标到 Android mipmap ...");
  run("npm", ["exec", "tauri", "icon", "--", iconPng]);
  const resDir = path.join(genAndroid, "app", "src", "main", "res");
  const bgXml = path.join(resDir, "drawable", "ic_launcher_background.xml");
  if (fs.existsSync(bgXml)) {
    fs.writeFileSync(
      bgXml,
      `<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android" android:shape="rectangle">
    <solid android:color="#1a1620" />
</shape>
`,
      "utf8",
    );
  }
  const fgXml = path.join(resDir, "drawable-v24", "ic_launcher_foreground.xml");
  if (fs.existsSync(fgXml)) {
    try {
      fs.unlinkSync(fgXml);
    } catch {
      /* ignore */
    }
  }
  console.log("[android-setup] Android 图标已更新");
}

/** 局域网 LLM（HTTP）需要 cleartext */
function patchCleartextNetworking() {
  if (fs.existsSync(manifestPath)) {
    let xml = fs.readFileSync(manifestPath, "utf8");
    if (!xml.includes('android:usesCleartextTraffic="true"')) {
      xml = xml.replace(
        /android:usesCleartextTraffic="\$\{usesCleartextTraffic\}"/,
        'android:usesCleartextTraffic="true"',
      );
      xml = xml.replace(
        /android:usesCleartextTraffic="false"/,
        'android:usesCleartextTraffic="true"',
      );
    }
    if (!xml.includes("networkSecurityConfig")) {
      xml = xml.replace(
        /android:usesCleartextTraffic="true"/,
        'android:usesCleartextTraffic="true"\n        android:networkSecurityConfig="@xml/network_security_config"',
      );
    }
    fs.writeFileSync(manifestPath, xml, "utf8");
    console.log("[android-setup] 已启用 cleartext + networkSecurityConfig");
  }

  const nscDir = path.join(genAndroid, "app", "src", "main", "res", "xml");
  const nscPath = path.join(nscDir, "network_security_config.xml");
  fs.mkdirSync(nscDir, { recursive: true });
  if (!fs.existsSync(nscPath)) {
    fs.writeFileSync(
      nscPath,
      `<?xml version="1.0" encoding="utf-8"?>
<network-security-config>
    <base-config cleartextTrafficPermitted="true" />
    <domain-config cleartextTrafficPermitted="true">
        <domain includeSubdomains="true">localhost</domain>
        <domain includeSubdomains="true">tauri.localhost</domain>
        <domain includeSubdomains="true">127.0.0.1</domain>
    </domain-config>
</network-security-config>
`,
      "utf8",
    );
  }

  if (fs.existsSync(gradleKts)) {
    let g = fs.readFileSync(gradleKts, "utf8");
    if (
      g.includes('getByName("release")') &&
      !g.includes('manifestPlaceholders["usesCleartextTraffic"] = "true"')
    ) {
      g = g.replace(
        /getByName\("release"\)\s*\{/,
        `getByName("release") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"`,
      );
      fs.writeFileSync(gradleKts, g, "utf8");
      console.log("[android-setup] release 已允许 cleartext");
    }
  }
}

if (!isAndroidProjectReady()) {
  // 清理不完整目录（例如仅有 xml / example，无 build.gradle.kts）
  if (fs.existsSync(genAndroid)) {
    console.log("[android-setup] 清理不完整的 gen/android ...");
    fs.rmSync(genAndroid, { recursive: true, force: true });
  }
  run("npm", [
    "exec",
    "tauri",
    "android",
    "init",
    "--",
    "--ci",
    "--skip-targets-install",
  ]);
} else {
  console.log("[android-setup] 已存在完整 gen/android，跳过 init");
}

if (!fs.existsSync(manifestPath)) {
  console.warn(
    "[android-setup] 未找到 AndroidManifest.xml，请确认 init 成功后再运行本脚本",
  );
  process.exit(1);
}

patchResizeableActivity(manifestPath);
syncLauncherIcons();
patchCleartextNetworking();

console.log(
  "[android-setup] 完成。下一步: npm run android:dev / python build_android.py",
);
