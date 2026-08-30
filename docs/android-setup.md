# Android 构建与侧载说明

代码路径: `docs/android-setup.md`

对齐参考：`D:\KKFiles\KKProjects\ASC\asc_ai` 的 `build_android.py` + `scripts/android-setup.mjs`。

## 1. 推荐用法（自动引导工具链）

与星灵 Ai 相同：一条命令会检测/安装 JDK 17、Android SDK/NDK，初始化 `gen/android`，打 APK 并用 debug.keystore 签名（可侧载）。

```powershell
cd D:\KKFiles\KKProjects\Kinit\kk_novel_ai

# 仅 Android（自动引导 + 构建）
python build_android.py --no-bump

# 或经总脚本
python build.py --platform android --no-bump

# 只装工具链、不编译
npm run android:bootstrap
# 等价: python build_android.py --bootstrap-only
```

工具链默认目录：`%USERPROFILE%\.kk_novel_ai\android-toolchain\`  
可用环境变量 `KK_NOVEL_AI_ANDROID_TOOLCHAIN` 改路径。

产物：

- `dist/kk_novel_ai_<ver>_*-signed.apk`（可安装）
- `dist/android/kk_novel_ai_<ver>_arm64-v8a.apk`（稳定别名）

## 2. Windows 默认不再顺带编安卓

```powershell
# 默认只打 Windows EXE（避免缺 Android 工程时报错）
python build.py

# Windows + Android
python build.py --platform all
# 或
npm run build:all
```

## 3. 手动初始化 / 补丁

```powershell
npm run android:setup
```

脚本会：

1. 若缺少 `src-tauri/gen/android/app/build.gradle.kts`，清理残缺目录后执行 `tauri android init`
2. 同步 `src-tauri/icons/icon.png` 为启动图标
3. 开启 cleartext + `network_security_config`（局域网 HTTP LLM）

相关代码：

- [`scripts/android-setup.mjs`](../scripts/android-setup.mjs)
- [`build_android.py`](../build_android.py)
- [`build.py`](../build.py)
- [`src-tauri/tauri.android.conf.json`](../src-tauri/tauri.android.conf.json)

## 4. 本机依赖（可选，脚本可自动装）

- Node.js / npm、Rust
- `rustup target add aarch64-linux-android`
- JDK 17+、Android SDK（Platform / Build-Tools / Platform-Tools / NDK）

若已装 Android Studio，也可自行设 `ANDROID_HOME`；`build_android.py` 仍优先使用自管 toolchain 目录以保持可复现。

## 5. Release 商店签名（可选）

侧载默认用 **debug.keystore**。上架需自备 keystore：

```powershell
keytool -genkey -v -keystore C:\Keys\kk-novel-ai.jks -keyalg RSA -keysize 2048 -validity 10000 -alias kk-novel-ai
copy src-tauri\gen\android\keystore.properties.example src-tauri\gen\android\keystore.properties
```

按 Tauri 文档编辑 `gen/android/app/build.gradle.kts` 的 `signingConfigs.release`。  
`keystore.properties` 与 `.jks` **禁止提交**。

跳过 debug 签名：`python build_android.py --no-sign`

## 6. Windows 无开发者模式说明

`tauri android build` 若因符号链接失败，`build_android.py` 会回退：复制 `libkk_novel_ai_lib.so` 到 `jniLibs` 再 Gradle 打包（与 asc_ai 相同）。

## 7. 局域网 LLM

手机上的 `127.0.0.1` 指向手机自身。设置里请填：

- 电脑局域网 IP：`http://192.168.x.x:1234/v1`
- 或公网 HTTPS OpenAI 兼容地址

cleartext 已在 `android:setup` 中开启。

## 8. 作品迁移

手机端使用「导出备份 / 导入备份」（ZIP），不要依赖打开任意目录。
