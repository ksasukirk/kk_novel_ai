#!/usr/bin/env python3
"""
Kk Novel Ai Tauri 打包脚本（Windows EXE；可选 Android APK）

功能:
  1. 可选自动递增版本号
  2. 构建一次前端（frontend-dist）
  3. 默认只构建 Windows；`--platform android|all` 时委托 build_android.py
     （自动引导 JDK/SDK、init gen/android、签名侧载 APK，对齐 asc_ai）

代码路径: kk_novel_ai/build.py
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from datetime import datetime
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
SRC_TAURI_DIR = SCRIPT_DIR / "src-tauri"
FRONTEND_DIST_DIR = SCRIPT_DIR / "frontend-dist"
DIST_DIR = SCRIPT_DIR / "dist"
TAURI_CONF = SRC_TAURI_DIR / "tauri.conf.json"
CARGO_TOML = SRC_TAURI_DIR / "Cargo.toml"
PACKAGE_JSON = SCRIPT_DIR / "package.json"
ANDROID_GEN = SRC_TAURI_DIR / "gen" / "android"
KEYSTORE_PROPS = ANDROID_GEN / "keystore.properties"
KEYSTORE_EXAMPLE = ANDROID_GEN / "keystore.properties.example"


def _run_command(command: str, env: dict[str, str]) -> subprocess.CompletedProcess:
    print(f"[CMD] {command}")
    return subprocess.run(command, cwd=str(SCRIPT_DIR), env=env, shell=True)


def _has_local_vite() -> bool:
    candidates = [
        SCRIPT_DIR / "node_modules" / ".bin" / "vite",
        SCRIPT_DIR / "node_modules" / ".bin" / "vite.cmd",
        SCRIPT_DIR / "node_modules" / "vite",
    ]
    return any(path.exists() for path in candidates)


def _has_local_tauri_cli() -> bool:
    candidates = [
        SCRIPT_DIR / "node_modules" / ".bin" / "tauri",
        SCRIPT_DIR / "node_modules" / ".bin" / "tauri.cmd",
        SCRIPT_DIR / "node_modules" / "@tauri-apps" / "cli",
    ]
    return any(path.exists() for path in candidates)


def _ensure_npm_dependencies(env: dict[str, str]) -> None:
    if _has_local_tauri_cli() and _has_local_vite():
        return

    print("[INFO] 未检测到本地前端或 Tauri 依赖，开始执行 `npm install` ...")
    r = _run_command("npm install", env)
    if r.returncode != 0:
        raise RuntimeError("npm install 失败，无法继续执行构建")

    if not _has_local_vite():
        raise RuntimeError("已执行 npm install，但仍未找到 `vite`")
    if not _has_local_tauri_cli():
        raise RuntimeError("已执行 npm install，但仍未找到 `@tauri-apps/cli`")


def read_version() -> str:
    data = json.loads(TAURI_CONF.read_text(encoding="utf-8"))
    return data.get("version", "0.0.0")


def bump_patch(current: str) -> str:
    m = re.match(r"^(\d+)\.(\d+)\.(\d+)$", current)
    if not m:
        raise RuntimeError(f"版本号格式不符合 X.Y.Z: {current}")
    major, minor, patch = int(m.group(1)), int(m.group(2)), int(m.group(3))
    patch += 1
    if patch >= 100:
        patch = 0
        minor += 1
    return f"{major}.{minor}.{patch}"


def write_version(new_ver: str) -> None:
    conf_data = json.loads(TAURI_CONF.read_text(encoding="utf-8"))
    conf_data["version"] = new_ver
    TAURI_CONF.write_text(
        json.dumps(conf_data, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )

    cargo_text = CARGO_TOML.read_text(encoding="utf-8")
    cargo_text = re.sub(
        r'^(version\s*=\s*")[^"]+(")(\s*$)',
        lambda m: f'{m.group(1)}{new_ver}{m.group(2)}{m.group(3)}',
        cargo_text,
        count=1,
        flags=re.MULTILINE,
    )
    CARGO_TOML.write_text(cargo_text, encoding="utf-8")

    pkg = json.loads(PACKAGE_JSON.read_text(encoding="utf-8"))
    pkg["version"] = new_ver
    PACKAGE_JSON.write_text(
        json.dumps(pkg, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def _sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def _check_windows_prereqs() -> None:
    if shutil.which("cargo") is None:
        raise RuntimeError("未找到 cargo，请先安装 Rust")


def _android_sdk_root() -> Path | None:
    for key in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        val = os.environ.get(key, "").strip()
        if val:
            p = Path(val)
            if p.exists():
                return p
    local = Path(os.environ.get("LOCALAPPDATA", "")) / "Android" / "Sdk"
    if local.exists():
        return local
    return None


def _check_android_prereqs(require_signing: bool) -> None:
    """轻量提示；完整工具链由 build_android.py 引导。"""
    gradle = ANDROID_GEN / "app" / "build.gradle.kts"
    if not gradle.exists():
        print(
            "[INFO] 尚未完整初始化 Android 工程，将由 build_android.py / android:setup 自动处理"
        )
    sdk = _android_sdk_root()
    if sdk is not None:
        print(f"[INFO] 检测到系统 Android SDK: {sdk}")
    else:
        print("[INFO] 未检测到系统 ANDROID_HOME，build_android.py 将引导安装到 ~/.kk_novel_ai/")
    if require_signing and KEYSTORE_PROPS.exists():
        print(f"[INFO] 发现 release 签名配置: {KEYSTORE_PROPS}")


def _find_apk(configuration: str) -> Path | None:
    outputs = ANDROID_GEN / "app" / "build" / "outputs" / "apk"
    if not outputs.exists():
        # 兼容 build_android.py 已复制到 dist 的产物
        dist_android = DIST_DIR / "android"
        if dist_android.exists():
            signed = sorted(
                dist_android.glob("*signed*.apk"),
                key=lambda p: p.stat().st_mtime,
                reverse=True,
            )
            if signed:
                return signed[0]
            any_apk = sorted(
                dist_android.glob("*.apk"),
                key=lambda p: p.stat().st_mtime,
                reverse=True,
            )
            if any_apk:
                return any_apk[0]
        return None
    patterns = [
        "**/aarch64*/release/*.apk",
        "**/arm64*/release/*.apk",
        "**/universal*/release/*.apk",
        "**/release/*.apk",
    ]
    if configuration == "debug":
        patterns = [
            "**/aarch64*/debug/*.apk",
            "**/arm64*/debug/*.apk",
            "**/universal*/debug/*.apk",
            "**/debug/*.apk",
        ]
    candidates: list[Path] = []
    for pat in patterns:
        candidates.extend(outputs.glob(pat))
    candidates = [p for p in candidates if p.is_file() and "unsigned" not in p.name.lower()]
    if not candidates:
        candidates = [p for p in outputs.rglob("*.apk") if p.is_file()]
    if not candidates:
        return None
    candidates.sort(key=lambda p: p.stat().st_mtime, reverse=True)
    return candidates[0]


def build_windows(env: dict[str, str], configuration: str, version: str) -> Path:
    _check_windows_prereqs()
    # 跳过 MSI/NSIS 打包（需要 .ico）；只产出可执行文件
    cmd = "npm run tauri -- build --no-bundle"
    if configuration == "debug":
        cmd = "npm run tauri -- build --debug --no-bundle"
    r = _run_command(cmd, env)
    if r.returncode != 0:
        raise RuntimeError("Windows Tauri 构建失败")

    candidates = [
        SRC_TAURI_DIR / "target" / configuration / "kk_novel_ai.exe",
    ]
    cargo_target = os.environ.get("CARGO_TARGET_DIR", "").strip()
    if cargo_target:
        candidates.insert(0, Path(cargo_target) / configuration / "kk_novel_ai.exe")
    # Cursor sandbox 偶发把产物写到临时 CARGO_TARGET_DIR
    temp_candidates = list(Path(os.environ.get("TEMP", "")).glob("**/release/kk_novel_ai.exe")) if configuration == "release" else []
    for p in temp_candidates[:5]:
        candidates.append(p)

    src_exe = next((p for p in candidates if p.exists()), None)
    if src_exe is None:
        raise RuntimeError(
            "未找到主程序产物 kk_novel_ai.exe，已查找: "
            + "; ".join(str(p) for p in candidates[:8])
        )

    DIST_DIR.mkdir(parents=True, exist_ok=True)
    out = DIST_DIR / f"kk_novel_ai_{version}.exe"
    shutil.copy2(str(src_exe), str(out))
    print(f"[OK] Windows: {out}")
    print(f"[INFO] SHA-256: {_sha256_file(out)}")
    return out


def build_android(
    env: dict[str, str],
    configuration: str,
    version: str,
    target: str,
) -> Path:
    """委托 build_android.py（JDK/SDK 引导 + init + 签名），对齐 asc_ai。"""
    _ = target  # 目前固定 aarch64，与 build_android.py 一致
    require_signing = configuration == "release"
    _check_android_prereqs(require_signing=require_signing)

    cmd = "python build_android.py --no-bump --no-frontend"
    if configuration == "debug":
        cmd += " --debug"
    r = _run_command(cmd, env)
    if r.returncode != 0:
        raise RuntimeError("Android 构建失败（见 build_android.py 输出）")

    apk = _find_apk(configuration)
    out_dir = DIST_DIR / "android"
    out_dir.mkdir(parents=True, exist_ok=True)
    out = out_dir / f"kk_novel_ai_{version}_arm64-v8a.apk"
    if apk is not None and apk.resolve() != out.resolve():
        shutil.copy2(str(apk), str(out))
        print(f"[OK] Android: {out} (from {apk})")
        print(f"[INFO] SHA-256: {_sha256_file(out)}")
        return out
    if out.exists():
        print(f"[OK] Android: {out}")
        print(f"[INFO] SHA-256: {_sha256_file(out)}")
        return out
    # build_android 已写到 dist/ 根目录时回退
    candidates = sorted(
        DIST_DIR.glob(f"kk_novel_ai_{version}_*-signed.apk"),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    if candidates:
        shutil.copy2(str(candidates[0]), str(out))
        print(f"[OK] Android: {out} (from {candidates[0]})")
        print(f"[INFO] SHA-256: {_sha256_file(out)}")
        return out
    raise RuntimeError(
        "未找到 Android APK 产物，请检查 dist/ 与 src-tauri/gen/android/app/build/outputs/apk"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Kk Novel Ai 双端打包脚本")
    parser.add_argument("--configuration", choices=["debug", "release"], default="release")
    parser.add_argument("--no-bump", action="store_true")
    parser.add_argument("--build-version", default="")
    parser.add_argument("--build-date", default="")
    parser.add_argument("--no-frontend", action="store_true")
    parser.add_argument(
        "--platform",
        choices=["windows", "android", "all"],
        default="windows",
        help="构建目标平台（默认 windows；android/all 走 build_android.py 自动引导）",
    )
    parser.add_argument("--android-target", default="aarch64")
    parser.add_argument("--skip-android", action="store_true", help="兼容别名：等同 --platform windows")
    parser.add_argument("--android-debug", action="store_true", help="Android 使用 debug 配置")
    args = parser.parse_args()

    platform = args.platform
    if args.skip_android:
        platform = "windows"

    android_configuration = "debug" if args.android_debug else args.configuration

    current_ver = read_version()
    if args.build_version.strip():
        version = args.build_version.strip()
        write_version(version)
    elif args.no_bump:
        version = current_ver
    else:
        version = bump_patch(current_ver)
        write_version(version)

    build_date = args.build_date.strip() or datetime.now().strftime("%Y%m%d-%H%M%S")

    env = os.environ.copy()
    env["BUILD_DATE"] = build_date
    env["BUILD_VERSION"] = version

    _ensure_npm_dependencies(env)

    if not args.no_frontend:
        r = _run_command("npm run frontend:build", env)
        if r.returncode != 0:
            raise RuntimeError("前端构建失败")

    index_html = FRONTEND_DIST_DIR / "index.html"
    if not index_html.exists():
        raise RuntimeError(f"缺少前端产物: {index_html}")

    outputs: list[Path] = []
    if platform in ("windows", "all"):
        outputs.append(build_windows(env, args.configuration, version))
    if platform in ("android", "all"):
        outputs.append(
            build_android(env, android_configuration, version, args.android_target)
        )

    print("[DONE] 构建完成：")
    for p in outputs:
        print(f"  - {p}")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(130)
    except Exception as e:
        print(f"[ERROR] {e}", file=sys.stderr)
        sys.exit(1)
