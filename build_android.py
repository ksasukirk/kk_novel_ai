#!/usr/bin/env python3
"""
Kk Novel Ai Android 构建脚本（自动引导 JDK + Android SDK + Tauri APK）

功能:
  1. 检测/安装 JDK 17（优先 winget Microsoft.OpenJDK.17）
  2. 自动检测 Android SDK：已齐全则跳过，缺失则下载安装
  3. 配置 JAVA_HOME / ANDROID_HOME / NDK_HOME
  4. 初始化 gen/android 并执行 tauri android build
  5. 将 APK 复制到 dist/，并用 Android debug.keystore 签名（默认可侧载）

用法:
  python build_android.py                         # 默认：自动检测 SDK，缺则装、齐则跳过
  python build_android.py --force-sdk-install     # 强制重装/更新 SDK 组件
  python build_android.py --skip-sdk-install      # 强制跳过安装（即便组件缺失）
  python build_android.py --aab --no-bump
  python build_android.py --no-sign               # 仅复制 unsigned，不签名
  python build_android.py --bootstrap-only        # 只引导工具链

产物（默认）: dist/kk_novel_ai_<ver>_app-arm64-release-signed.apk
代码路径: kk_novel_ai/build_android.py
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import zipfile
from datetime import datetime
from pathlib import Path
from urllib.request import urlretrieve

SCRIPT_DIR = Path(__file__).resolve().parent
SRC_TAURI_DIR = SCRIPT_DIR / "src-tauri"
FRONTEND_DIST_DIR = SCRIPT_DIR / "frontend-dist"
DIST_DIR = SCRIPT_DIR / "dist"
GEN_ANDROID = SRC_TAURI_DIR / "gen" / "android"
TAURI_CONF = SRC_TAURI_DIR / "tauri.conf.json"
PACKAGE_JSON = SCRIPT_DIR / "package.json"
CARGO_TOML = SRC_TAURI_DIR / "Cargo.toml"

# 工具链默认落在用户本地，避免写入系统目录需管理员权限
TOOLCHAIN_ROOT = Path(
    os.environ.get("KK_NOVEL_AI_ANDROID_TOOLCHAIN")
    or (Path.home() / ".kk_novel_ai" / "android-toolchain")
)
JDK_DIR = TOOLCHAIN_ROOT / "jdk-17"
SDK_DIR = TOOLCHAIN_ROOT / "sdk"
DOWNLOAD_DIR = TOOLCHAIN_ROOT / "downloads"

# Google cmdline-tools（Windows）。若失效可改 KK_NOVEL_AI_CMDLINE_TOOLS_URL
CMDLINE_TOOLS_URL = os.environ.get(
    "KK_NOVEL_AI_CMDLINE_TOOLS_URL",
    "https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip",
)

# 与 tauri.conf.json bundle.android.minSdkVersion(26) 对齐；编译用较新 platform
ANDROID_API = os.environ.get("KK_NOVEL_AI_ANDROID_API", "34")
BUILD_TOOLS = os.environ.get("KK_NOVEL_AI_ANDROID_BUILD_TOOLS", "34.0.0")
# side-by-side NDK；可用环境变量覆盖
NDK_PACKAGE = os.environ.get("KK_NOVEL_AI_ANDROID_NDK", "ndk;26.1.10909125")

SDK_PACKAGES = [
    "platform-tools",
    f"platforms;android-{ANDROID_API}",
    f"build-tools;{BUILD_TOOLS}",
    NDK_PACKAGE,
]


def log(msg: str) -> None:
    print(f"[android-build] {msg}", flush=True)


def run(
    command: str | list[str],
    *,
    env: dict[str, str] | None = None,
    cwd: Path | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess:
    if isinstance(command, list):
        display = " ".join(command)
        shell = False
        args: str | list[str] = command
    else:
        display = command
        shell = True
        args = command
    log(f"$ {display}")
    r = subprocess.run(
        args,
        cwd=str(cwd or SCRIPT_DIR),
        env=env or os.environ.copy(),
        shell=shell,
    )
    if check and r.returncode != 0:
        raise RuntimeError(f"命令失败 ({r.returncode}): {display}")
    return r


def which(name: str) -> Path | None:
    p = shutil.which(name)
    return Path(p) if p else None


def find_java_home() -> Path | None:
    env_home = os.environ.get("JAVA_HOME")
    if env_home:
        home = Path(env_home)
        if (home / "bin" / "java.exe").exists() or (home / "bin" / "java").exists():
            return home

    java = which("java")
    if java:
        # java -> .../bin/java.exe -> parent.parent = JAVA_HOME
        try:
            resolved = java.resolve()
            return resolved.parent.parent
        except OSError:
            pass

    candidates = [
        JDK_DIR,
        Path(r"C:\Program Files\Microsoft\jdk-17*"),
        Path(r"C:\Program Files\Eclipse Adoptium\jdk-17*"),
        Path(r"C:\Program Files\Java\jdk-17*"),
        Path(r"C:\Program Files\Android\Android Studio\jbr"),
    ]
    for pattern in candidates:
        if "*" in str(pattern):
            parent = pattern.parent
            glob = pattern.name
            if parent.is_dir():
                for hit in sorted(parent.glob(glob), reverse=True):
                    if (hit / "bin" / "java.exe").exists():
                        return hit
        elif pattern.is_dir() and (
            (pattern / "bin" / "java.exe").exists() or (pattern / "bin" / "java").exists()
        ):
            return pattern
    return None


def ensure_jdk() -> Path:
    existing = find_java_home()
    if existing:
        log(f"已找到 JDK: {existing}")
        return existing

    log("未找到 JDK，尝试 winget 安装 Microsoft.OpenJDK.17 ...")
    winget = which("winget")
    if not winget:
        raise RuntimeError(
            "未找到 JDK 且无 winget。请手动安装 JDK 17 并设置 JAVA_HOME，"
            "或安装 winget 后重试。"
        )

    r = run(
        [
            str(winget),
            "install",
            "-e",
            "--id",
            "Microsoft.OpenJDK.17",
            "--accept-package-agreements",
            "--accept-source-agreements",
        ],
        check=False,
    )
    if r.returncode not in (0, -1978335189):  # -1978335189 = already installed
        # winget 成功码也可能是其它，再探测一次
        pass

    # winget 安装后 PATH 可能未刷新，按常见路径扫描
    for base in (
        Path(r"C:\Program Files\Microsoft"),
        Path(r"C:\Program Files\Eclipse Adoptium"),
        Path(r"C:\Program Files\Java"),
    ):
        if not base.is_dir():
            continue
        for hit in sorted(base.glob("jdk-17*"), reverse=True):
            if (hit / "bin" / "java.exe").exists():
                log(f"JDK 已就绪: {hit}")
                return hit

    existing = find_java_home()
    if existing:
        return existing
    raise RuntimeError("winget 安装 JDK 后仍无法定位 JAVA_HOME，请重启终端后重试")


def download_file(url: str, dest: Path) -> Path:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.is_file() and dest.stat().st_size > 1_000_000:
        log(f"使用已缓存下载: {dest}")
        return dest
    log(f"下载: {url}")
    tmp = dest.with_suffix(dest.suffix + ".partial")
    try:
        urlretrieve(url, str(tmp))
        tmp.replace(dest)
    except Exception:
        if tmp.exists():
            tmp.unlink(missing_ok=True)
        raise
    return dest


def ensure_cmdline_tools(sdk_root: Path) -> Path:
    """返回 sdkmanager 路径。"""
    sdkmanager = (
        sdk_root / "cmdline-tools" / "latest" / "bin" / "sdkmanager.bat"
        if os.name == "nt"
        else sdk_root / "cmdline-tools" / "latest" / "bin" / "sdkmanager"
    )
    if sdkmanager.is_file():
        return sdkmanager

    zip_path = download_file(
        CMDLINE_TOOLS_URL,
        DOWNLOAD_DIR / "commandlinetools-win_latest.zip",
    )
    extract_tmp = DOWNLOAD_DIR / "cmdline-tools-extract"
    if extract_tmp.exists():
        shutil.rmtree(extract_tmp)
    extract_tmp.mkdir(parents=True)
    log(f"解压 cmdline-tools -> {extract_tmp}")
    with zipfile.ZipFile(zip_path, "r") as zf:
        zf.extractall(extract_tmp)

    # zip 内通常为 cmdline-tools/{bin,lib,...}
    src = extract_tmp / "cmdline-tools"
    if not src.is_dir():
        # 兼容顶层即为内容
        children = [p for p in extract_tmp.iterdir() if p.is_dir()]
        if len(children) == 1:
            src = children[0]
        else:
            raise RuntimeError(f"无法识别 cmdline-tools 解压结构: {extract_tmp}")

    dest = sdk_root / "cmdline-tools" / "latest"
    if dest.exists():
        shutil.rmtree(dest)
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(str(src), str(dest))
    shutil.rmtree(extract_tmp, ignore_errors=True)

    if not sdkmanager.is_file():
        raise RuntimeError(f"解压后未找到 sdkmanager: {sdkmanager}")
    return sdkmanager


def accept_licenses(sdkmanager: Path, env: dict[str, str]) -> None:
    log("接受 Android SDK 许可协议 ...")
    # 向 sdkmanager --licenses 连续喂 y
    if os.name == "nt":
        cmd = f'cmd /c "yes 2>nul | ""{sdkmanager}"" --sdk_root={SDK_DIR} --licenses"'
    else:
        cmd = f'yes | "{sdkmanager}" --sdk_root={SDK_DIR} --licenses'
    # yes 在 Windows 可能不存在，用 PowerShell 循环
    if os.name == "nt" and not which("yes"):
        ps = (
            f"$p = Start-Process -FilePath '{sdkmanager}' "
            f"-ArgumentList '--sdk_root={SDK_DIR}','--licenses' "
            f"-RedirectStandardInput 'CONIN$' -NoNewWindow -PassThru -Wait; "
            # 更可靠：用 echo y 管道
        )
        # 使用 Python 管道
        proc = subprocess.Popen(
            [str(sdkmanager), f"--sdk_root={SDK_DIR}", "--licenses"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            env=env,
            text=True,
        )
        assert proc.stdin is not None
        try:
            # 足够多的 y 覆盖所有许可
            proc.stdin.write("y\n" * 200)
            proc.stdin.close()
        except BrokenPipeError:
            pass
        out, _ = proc.communicate(timeout=600)
        if out:
            # 只打印末尾，避免刷屏
            lines = out.strip().splitlines()
            for line in lines[-20:]:
                print(line)
        return

    run(cmd, env=env, check=False)


def install_sdk_packages(sdkmanager: Path, env: dict[str, str]) -> None:
    accept_licenses(sdkmanager, env)
    log(f"安装 SDK 组件: {', '.join(SDK_PACKAGES)}")
    args = [str(sdkmanager), f"--sdk_root={SDK_DIR}", "--install", *SDK_PACKAGES]
    proc = subprocess.Popen(
        args,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        env=env,
        text=True,
    )
    assert proc.stdin is not None
    try:
        proc.stdin.write("y\n" * 50)
        proc.stdin.close()
    except BrokenPipeError:
        pass
    assert proc.stdout is not None
    for line in proc.stdout:
        print(line, end="")
    code = proc.wait()
    if code != 0:
        raise RuntimeError(f"sdkmanager 安装失败 ({code})")


def resolve_ndk_home(sdk_root: Path) -> Path:
    ndk_root = sdk_root / "ndk"
    if not ndk_root.is_dir():
        raise RuntimeError(f"未找到 NDK 目录: {ndk_root}")
    versions = sorted(
        [p for p in ndk_root.iterdir() if p.is_dir()],
        key=lambda p: p.name,
        reverse=True,
    )
    if not versions:
        raise RuntimeError(f"NDK 目录为空: {ndk_root}")
    return versions[0]


def build_env(java_home: Path, sdk_root: Path, ndk_home: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["JAVA_HOME"] = str(java_home)
    env["ANDROID_HOME"] = str(sdk_root)
    env["ANDROID_SDK_ROOT"] = str(sdk_root)
    env["NDK_HOME"] = str(ndk_home)
    # 固定到仓库 target，避免落在沙箱 Temp 导致后续路径混乱
    env["CARGO_TARGET_DIR"] = str(SCRIPT_DIR / "target")
    path_parts = [
        str(java_home / "bin"),
        str(sdk_root / "cmdline-tools" / "latest" / "bin"),
        str(sdk_root / "platform-tools"),
        env.get("PATH", ""),
    ]
    env["PATH"] = os.pathsep.join(path_parts)
    env["CI"] = "true"
    return env


ABI_TO_JNI = {
    "aarch64-linux-android": "arm64-v8a",
    "armv7-linux-androideabi": "armeabi-v7a",
    "i686-linux-android": "x86",
    "x86_64-linux-android": "x86_64",
}


def find_release_lib(triple: str, release: bool) -> Path | None:
    profile = "release" if release else "debug"
    name = "libkk_novel_ai_lib.so"
    candidates = [
        SCRIPT_DIR / "target" / triple / profile / name,
        SRC_TAURI_DIR / "target" / triple / profile / name,
    ]
    # Cursor / 沙箱偶发把 CARGO_TARGET_DIR 指到 Temp
    temp = Path(os.environ.get("TEMP", os.environ.get("TMP", "")))
    if temp:
        for p in temp.glob(f"**/cargo-target/{triple}/{profile}/{name}"):
            candidates.append(p)
        for p in temp.glob(
            f"**/cursor-sandbox-cache/**/cargo-target/{triple}/{profile}/{name}"
        ):
            candidates.append(p)
    for c in candidates:
        if c.is_file():
            return c
    return None


def copy_native_libs_to_jni(*, release: bool, triples: list[str]) -> int:
    """Windows 无开发者模式时 tauri 无法创建符号链接，改为复制 .so。"""
    copied = 0
    for triple in triples:
        abi = ABI_TO_JNI.get(triple)
        if not abi:
            continue
        lib = find_release_lib(triple, release)
        if not lib:
            log(f"未找到原生库: {triple}")
            continue
        dest_dir = (
            GEN_ANDROID / "app" / "src" / "main" / "jniLibs" / abi
        )
        dest_dir.mkdir(parents=True, exist_ok=True)
        dest = dest_dir / "libkk_novel_ai_lib.so"
        shutil.copy2(lib, dest)
        log(f"已复制原生库 -> {dest} ({dest.stat().st_size} bytes)")
        copied += 1
    return copied


def stop_gradle_daemons(env: dict[str, str]) -> None:
    gradle = GEN_ANDROID / ("gradlew.bat" if os.name == "nt" else "gradlew")
    if not gradle.is_file():
        return
    log("停止 Gradle Daemon，避免 Windows 文件锁 ...")
    subprocess.run(
        [str(gradle), "--stop"],
        cwd=str(GEN_ANDROID),
        env=env,
        capture_output=True,
        text=True,
    )


def clean_gradle_lock_hotspots() -> None:
    """清理易被锁定的 R8/dex 中间产物。"""
    hotspots = [
        GEN_ANDROID / "app" / "build" / "intermediates" / "dex",
        GEN_ANDROID / "app" / "build" / "intermediates" / "r8",
        GEN_ANDROID / "app" / "build" / "intermediates" / "shrunk_classes",
    ]
    for p in hotspots:
        if p.exists():
            try:
                shutil.rmtree(p)
                log(f"已清理: {p}")
            except OSError as e:
                log(f"清理失败（可忽略）: {p}: {e}")


def patch_release_minify_off() -> None:
    """Windows 上 R8 minify 易因文件锁失败；本地 APK 默认关闭压缩混淆。"""
    gradle_kts = GEN_ANDROID / "app" / "build.gradle.kts"
    if not gradle_kts.is_file():
        return
    text = gradle_kts.read_text(encoding="utf-8")
    if "isMinifyEnabled = true" not in text:
        return
    updated = text.replace(
        "isMinifyEnabled = true",
        "isMinifyEnabled = false // kk_novel_ai: Windows 本地构建关闭 R8，避免 dex 文件锁",
        1,
    )
    if updated != text:
        gradle_kts.write_text(updated, encoding="utf-8")
        log("已关闭 release minify（规避 Windows R8 文件锁）")


def gradle_assemble(env: dict[str, str], *, aab: bool, release: bool) -> None:
    """在已有 jniLibs 的前提下打包，跳过 rustBuild（避免再次触发 symlink）。"""
    gradle = GEN_ANDROID / ("gradlew.bat" if os.name == "nt" else "gradlew")
    if not gradle.is_file():
        raise RuntimeError(f"未找到 Gradle Wrapper: {gradle}")
    task = ":app:bundleRelease" if aab and release else (
        ":app:assembleArm64Release" if release else ":app:assembleArm64Debug"
    )
    if aab and not release:
        task = ":app:bundleDebug"
    if aab and release:
        # AAB 需全量或至少 arm64；仍跳过 rustBuild
        task = ":app:bundleRelease"
    # 跳过 rustBuild*，直接用已复制的 jniLibs（Windows 无 symlink 时必需）
    # 注意：排除不存在的 task 会导致 Gradle 失败，故仅排除实际存在的名称
    excludes = [
        "rustBuildArm64Release",
        "rustBuildArmRelease",
        "rustBuildX86Release",
        "rustBuildX86_64Release",
        "rustBuildUniversalRelease",
        "rustBuildArm64Debug",
        "rustBuildArmDebug",
        "rustBuildX86Debug",
        "rustBuildX86_64Debug",
        "rustBuildUniversalDebug",
    ]
    cmd = [str(gradle), task, "--no-daemon"]
    for x in excludes:
        cmd.extend(["-x", x])

    patch_release_minify_off()
    last_err = None
    for attempt in range(1, 3):
        stop_gradle_daemons(env)
        clean_gradle_lock_hotspots()
        log(f"Gradle 打包（跳过 rustBuild，使用已复制 jniLibs）第 {attempt} 次 ...")
        r = subprocess.run(cmd, cwd=str(GEN_ANDROID), env=env)
        if r.returncode == 0:
            return
        last_err = r.returncode
        log(f"Gradle 失败 ({r.returncode})，准备重试 ...")
    raise RuntimeError(f"Gradle 打包失败 ({last_err})")


def tauri_android_build(env: dict[str, str], *, aab: bool, debug: bool) -> None:
    """调用 tauri android build；若因 Windows 符号链接失败则回退到 copy + gradle。"""
    fmt = "--aab" if aab else "--apk"
    # Cargo default 已不含 desktop；Android 不传 --features desktop
    cmd = f"npm exec tauri android build -- {fmt} --target aarch64"
    if debug:
        cmd += " --debug"
    r = run(cmd, env=env, check=False)
    if r.returncode == 0:
        return

    log("tauri android build 失败，尝试 Windows 无 symlink 回退路径 ...")
    n = copy_native_libs_to_jni(release=not debug, triples=["aarch64-linux-android"])
    if n == 0:
        raise RuntimeError(
            "tauri 构建失败且未找到 libkk_novel_ai_lib.so。"
            "若错误涉及 symbolic link，请开启 Windows 开发者模式后重试，"
            "或确认 Rust Android 目标已编译成功。"
        )
    gradle_assemble(env, aab=aab, release=not debug)


def read_version() -> str:
    return json.loads(TAURI_CONF.read_text(encoding="utf-8")).get("version", "0.0.0")


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
    conf = json.loads(TAURI_CONF.read_text(encoding="utf-8"))
    conf["version"] = new_ver
    TAURI_CONF.write_text(
        json.dumps(conf, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )

    cargo = CARGO_TOML.read_text(encoding="utf-8")
    cargo = re.sub(
        r'^(version\s*=\s*")[^"]+(")(\s*$)',
        lambda m: f"{m.group(1)}{new_ver}{m.group(2)}{m.group(3)}",
        cargo,
        count=1,
        flags=re.MULTILINE,
    )
    CARGO_TOML.write_text(cargo, encoding="utf-8")

    pkg = json.loads(PACKAGE_JSON.read_text(encoding="utf-8"))
    pkg["version"] = new_ver
    PACKAGE_JSON.write_text(
        json.dumps(pkg, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


def ensure_npm(env: dict[str, str]) -> None:
    vite = SCRIPT_DIR / "node_modules" / "vite"
    tauri = SCRIPT_DIR / "node_modules" / "@tauri-apps" / "cli"
    if vite.exists() and tauri.exists():
        return
    run("npm install", env=env)


def ensure_android_project(env: dict[str, str]) -> None:
    gradle = GEN_ANDROID / "app" / "build.gradle.kts"
    if gradle.is_file():
        log("已存在完整 gen/android（仍会跑 android:setup 补丁）")
    else:
        log("gen/android 不完整或不存在，将由 android:setup 初始化")
    run("npm run android:setup", env=env)


def ensure_debug_keystore() -> Path:
    """确保 ~/.android/debug.keystore 存在（与 Android Studio debug 密钥一致）。"""
    ks = Path.home() / ".android" / "debug.keystore"
    if ks.is_file():
        return ks
    ks.parent.mkdir(parents=True, exist_ok=True)
    java = find_java_home()
    if not java:
        raise RuntimeError("无法生成 debug.keystore：未找到 JDK（JAVA_HOME）")
    keytool = java / "bin" / ("keytool.exe" if os.name == "nt" else "keytool")
    if not keytool.is_file():
        raise RuntimeError(f"未找到 keytool: {keytool}")
    log(f"生成 Android debug.keystore: {ks}")
    r = subprocess.run(
        [
            str(keytool),
            "-genkeypair",
            "-keystore",
            str(ks),
            "-storepass",
            "android",
            "-alias",
            "androiddebugkey",
            "-keypass",
            "android",
            "-keyalg",
            "RSA",
            "-keysize",
            "2048",
            "-validity",
            "10000",
            "-dname",
            "CN=Android Debug,O=Android,C=US",
        ],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0 or not ks.is_file():
        raise RuntimeError(f"生成 debug.keystore 失败: {r.stderr or r.stdout}")
    return ks


def find_apksigner() -> Path:
    build_tools = SDK_DIR / "build-tools"
    if not build_tools.is_dir():
        raise RuntimeError(f"未找到 build-tools: {build_tools}（请先完成 SDK 引导）")
    name = "apksigner.bat" if os.name == "nt" else "apksigner"
    for p in sorted(build_tools.glob(f"*/{name}"), reverse=True):
        return p
    raise RuntimeError(f"未找到 apksigner（请安装 build-tools;{BUILD_TOOLS}）")


def java_env() -> dict[str, str]:
    """为 apksigner/keytool 注入 JAVA_HOME（Windows 上 bat 包装脚本依赖它）。"""
    env = os.environ.copy()
    java_home = find_java_home()
    if not java_home:
        raise RuntimeError("未找到 JDK：签名需要 JAVA_HOME（请先完成工具链引导）")
    env["JAVA_HOME"] = str(java_home)
    env["PATH"] = str(java_home / "bin") + os.pathsep + env.get("PATH", "")
    return env


def signed_apk_path_for(apk_path: Path) -> Path:
    """由 unsigned（或任意）APK 路径推导 *-signed.apk 路径。"""
    stem = apk_path.stem
    if stem.endswith("-signed"):
        return apk_path
    base = stem[: -len("-unsigned")] if stem.endswith("-unsigned") else stem
    return apk_path.with_name(f"{base}-signed.apk")


def sign_apk_debug(apk_path: Path) -> Path:
    """用 Android debug.keystore 签名，产出可侧载安装的 *-signed.apk。"""
    if apk_path.suffix.lower() != ".apk":
        raise RuntimeError(f"只能签名 APK: {apk_path}")
    apksigner = find_apksigner()
    ks = ensure_debug_keystore()
    env = java_env()

    out = signed_apk_path_for(apk_path)
    if out.resolve() != apk_path.resolve():
        shutil.copy2(apk_path, out)

    r = subprocess.run(
        [
            str(apksigner),
            "sign",
            "--ks",
            str(ks),
            "--ks-pass",
            "pass:android",
            "--key-pass",
            "pass:android",
            "--ks-key-alias",
            "androiddebugkey",
            str(out),
        ],
        capture_output=True,
        text=True,
        env=env,
    )
    if r.returncode != 0:
        raise RuntimeError(f"debug 签名失败: {r.stderr or r.stdout}")

    # 校验签名，避免产出无法安装的包
    v = subprocess.run(
        [str(apksigner), "verify", "--print-certs", str(out)],
        capture_output=True,
        text=True,
        env=env,
    )
    if v.returncode != 0:
        raise RuntimeError(f"签名校验失败: {v.stderr or v.stdout}")

    log(f"可安装包（已 debug 签名）: {out}")
    return out


def copy_artifacts(version: str, *, sign: bool = True) -> None:
    DIST_DIR.mkdir(parents=True, exist_ok=True)
    android_out = DIST_DIR / "android"
    android_out.mkdir(parents=True, exist_ok=True)
    bases = [
        GEN_ANDROID / "app" / "build" / "outputs" / "apk",
        GEN_ANDROID / "app" / "build" / "outputs" / "bundle",
    ]
    copied = 0
    apk_copied = 0
    signed_paths: list[Path] = []
    for base in bases:
        if not base.is_dir():
            continue
        for path in base.rglob("*"):
            if path.suffix.lower() in {".apk", ".aab"} and path.is_file():
                out = DIST_DIR / f"kk_novel_ai_{version}_{path.name}"
                shutil.copy2(path, out)
                log(f"已输出: {out}")
                # 兼容旧路径 dist/android/
                shutil.copy2(path, android_out / out.name)
                copied += 1
                if path.suffix.lower() == ".apk":
                    apk_copied += 1
                    if sign:
                        signed = sign_apk_debug(out)
                        signed_paths.append(signed)
                        shutil.copy2(signed, android_out / signed.name)
                        # 稳定别名，便于侧载
                        alias = android_out / f"kk_novel_ai_{version}_arm64-v8a.apk"
                        shutil.copy2(signed, alias)
                        log(f"侧载别名: {alias}")
    if copied == 0:
        raise RuntimeError(
            "未在默认 outputs 找到 APK/AAB，请检查 src-tauri/gen/android/app/build/outputs"
        )
    if sign and apk_copied > 0 and not signed_paths:
        raise RuntimeError("默认需要对 APK 签名，但签名步骤未产出 *-signed.apk")
    if sign and apk_copied == 0:
        log("提示: 本次无 APK（可能仅有 AAB），跳过签名要求")


def sdk_components_status(sdk_root: Path) -> dict[str, bool]:
    """检测本机构建所需 SDK 组件是否就绪。"""
    platform_tools = sdk_root / "platform-tools"
    adb_name = "adb.exe" if os.name == "nt" else "adb"
    return {
        "platform-tools": (platform_tools / adb_name).is_file()
        or (platform_tools.is_dir() and any(platform_tools.iterdir())),
        f"platforms;android-{ANDROID_API}": (
            sdk_root / "platforms" / f"android-{ANDROID_API}"
        ).is_dir(),
        f"build-tools;{BUILD_TOOLS}": (sdk_root / "build-tools" / BUILD_TOOLS).is_dir(),
        "ndk": (sdk_root / "ndk").is_dir()
        and any((sdk_root / "ndk").iterdir()),
    }


def sdk_components_ready(sdk_root: Path) -> bool:
    status = sdk_components_status(sdk_root)
    return all(status.values())


def bootstrap(*, skip_sdk_install: bool = False, force_sdk_install: bool = False) -> dict[str, str]:
    TOOLCHAIN_ROOT.mkdir(parents=True, exist_ok=True)
    DOWNLOAD_DIR.mkdir(parents=True, exist_ok=True)
    SDK_DIR.mkdir(parents=True, exist_ok=True)

    java_home = ensure_jdk()
    # 临时 env 供 sdkmanager 使用
    env = os.environ.copy()
    env["JAVA_HOME"] = str(java_home)
    env["PATH"] = str(java_home / "bin") + os.pathsep + env.get("PATH", "")

    sdkmanager = ensure_cmdline_tools(SDK_DIR)
    status = sdk_components_status(SDK_DIR)
    ready = all(status.values())
    missing = [name for name, ok in status.items() if not ok]

    if force_sdk_install:
        log("强制安装/更新 SDK 组件 ...")
        for name, ok in status.items():
            log(f"  [{'OK' if ok else '缺'}] {name}")
        install_sdk_packages(sdkmanager, env)
    elif skip_sdk_install:
        if ready:
            log("跳过 SDK 组件安装（--skip-sdk-install，组件已齐全）")
        else:
            log(
                "警告: --skip-sdk-install 已指定，但缺少: "
                + ", ".join(missing)
                + "；继续构建，失败时请去掉该参数重跑"
            )
    elif ready:
        log(f"检测到 SDK 组件已齐全（{SDK_DIR}），跳过安装")
        for name in status:
            log(f"  [OK] {name}")
    else:
        log(f"检测到 SDK 组件缺失: {', '.join(missing)}，开始安装 ...")
        install_sdk_packages(sdkmanager, env)

    ndk_home = resolve_ndk_home(SDK_DIR)
    env = build_env(java_home, SDK_DIR, ndk_home)
    log(f"JAVA_HOME={env['JAVA_HOME']}")
    log(f"ANDROID_HOME={env['ANDROID_HOME']}")
    log(f"NDK_HOME={env['NDK_HOME']}")

    # 写入本机 env 文件，方便后续手动构建
    env_file = TOOLCHAIN_ROOT / "env.ps1"
    env_file.write_text(
        "\n".join(
            [
                f'$env:JAVA_HOME = "{java_home}"',
                f'$env:ANDROID_HOME = "{SDK_DIR}"',
                f'$env:ANDROID_SDK_ROOT = "{SDK_DIR}"',
                f'$env:NDK_HOME = "{ndk_home}"',
                f'$env:Path = "{java_home}\\bin;{SDK_DIR}\\cmdline-tools\\latest\\bin;{SDK_DIR}\\platform-tools;" + $env:Path',
                "",
            ]
        ),
        encoding="utf-8",
    )
    log(f"已写入环境脚本: {env_file} （可 . {env_file}）")
    return env


def main() -> None:
    parser = argparse.ArgumentParser(description="Kk Novel Ai Android 构建（含 SDK 引导）")
    parser.add_argument("--bootstrap-only", action="store_true", help="只安装 JDK/SDK")
    parser.add_argument(
        "--skip-sdk-install",
        action="store_true",
        help="强制跳过 sdkmanager 安装（默认已会在组件齐全时自动跳过）",
    )
    parser.add_argument(
        "--force-sdk-install",
        action="store_true",
        help="强制重装/更新 SDK 组件（忽略本地已存在检测）",
    )
    parser.add_argument("--no-bump", action="store_true")
    parser.add_argument("--build-version", default="")
    parser.add_argument("--no-frontend", action="store_true")
    parser.add_argument("--aab", action="store_true", help="产出 AAB 而非 APK")
    parser.add_argument("--debug", action="store_true", help="debug 构建")
    parser.add_argument(
        "--no-sign",
        action="store_true",
        help="跳过 debug 签名（默认会对 APK 签名并产出 *-signed.apk）",
    )
    args = parser.parse_args()
    if args.skip_sdk_install and args.force_sdk_install:
        parser.error("--skip-sdk-install 与 --force-sdk-install 不能同时使用")

    env = bootstrap(
        skip_sdk_install=args.skip_sdk_install,
        force_sdk_install=args.force_sdk_install,
    )
    if args.bootstrap_only:
        log("工具链引导完成")
        return

    current = read_version()
    if args.build_version.strip():
        version = args.build_version.strip()
        write_version(version)
    elif args.no_bump:
        version = current
    else:
        version = bump_patch(current)
        write_version(version)

    env["BUILD_VERSION"] = version
    env["BUILD_DATE"] = datetime.now().strftime("%Y%m%d-%H%M%S")

    ensure_npm(env)
    if not args.no_frontend:
        run("npm run frontend:build", env=env)
    if not (FRONTEND_DIST_DIR / "index.html").exists():
        raise RuntimeError(f"缺少前端产物: {FRONTEND_DIST_DIR / 'index.html'}")

    ensure_android_project(env)

    tauri_android_build(env, aab=args.aab, debug=args.debug)
    # 默认签名：侧载安装必须用 *-signed.apk；--no-sign 仅保留 unsigned
    copy_artifacts(version, sign=not args.no_sign)
    log("Android 构建完成")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(130)
    except Exception as e:
        log(f"失败: {e}")
        sys.exit(1)
