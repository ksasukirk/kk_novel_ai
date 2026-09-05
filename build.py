#!/usr/bin/env python3
"""
Kk Novel Ai Tauri 打包脚本（Windows EXE；可选 Android APK）

功能:
  1. 可选自动递增版本号
  2. 构建一次前端（frontend-dist）
  3. 默认只构建 Windows；`--platform android|all` 时委托 build_android.py
     （自动引导 JDK/SDK、init gen/android、签名侧载 APK，对齐 asc_ai）
  4. release 构建成功后默认用 Cursor Auto 模型按上次 tag 的 diff 重写 README.md，
     再 git add -A 提交全部工作区改动、push，并把 exe/apk 上传到 GitHub Release
     （`--no-github-release` 跳过整段；`--no-cursor-readme` 只跳过 Cursor；
       debug 默认不发版，除非 `--github-release`；
       gh 未登录时构建/git 仍完成，登录后用 `--publish-only` 补传）

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
import tempfile
from datetime import datetime
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
SRC_TAURI_DIR = SCRIPT_DIR / "src-tauri"
FRONTEND_DIST_DIR = SCRIPT_DIR / "frontend-dist"
DIST_DIR = SCRIPT_DIR / "dist"
TAURI_CONF = SRC_TAURI_DIR / "tauri.conf.json"
CARGO_TOML = SRC_TAURI_DIR / "Cargo.toml"
PACKAGE_JSON = SCRIPT_DIR / "package.json"
CARGO_LOCK = SRC_TAURI_DIR / "Cargo.lock"
ANDROID_GEN = SRC_TAURI_DIR / "gen" / "android"
KEYSTORE_PROPS = ANDROID_GEN / "keystore.properties"
KEYSTORE_EXAMPLE = ANDROID_GEN / "keystore.properties.example"
GH_REPO = "ksasukirk/kk_novel_ai"
GITHUB_URL = f"https://github.com/{GH_REPO}"
VERSION_FILES = (TAURI_CONF, CARGO_TOML, PACKAGE_JSON, CARGO_LOCK)
README_PATH = SCRIPT_DIR / "README.md"
RELEASE_PROMPT_TEMPLATE = SCRIPT_DIR / "scripts" / "release-readme-prompt.md"
DIFF_EXCLUDES = (
    ":(exclude)dist",
    ":(exclude)frontend-dist",
    ":(exclude)node_modules",
    ":(exclude)src-tauri/target",
)
SECRET_SUFFIXES = (".pem", ".keystore")
SECRET_NAMES = frozenset({"credentials.json"})
CURSOR_TIMEOUT_SECS = 600
MAX_DIFF_CHARS = 180_000
CURSOR_MODEL = "auto"


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


def _run_argv(
    argv: list[str],
    env: dict[str, str],
    *,
    timeout: int | None = None,
) -> subprocess.CompletedProcess:
    print("[CMD]", " ".join(argv))
    return subprocess.run(argv, cwd=str(SCRIPT_DIR), env=env, timeout=timeout)


def _run_argv_capture(argv: list[str], env: dict[str, str]) -> subprocess.CompletedProcess:
    print("[CMD]", " ".join(argv))
    return subprocess.run(
        argv,
        cwd=str(SCRIPT_DIR),
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )


def collect_dist_outputs(version: str) -> list[Path]:
    """已有 dist 产物（补发 Release 用）。代码路径: build.py"""
    found: list[Path] = []
    seen: set[str] = set()
    candidates = [
        DIST_DIR / f"kk_novel_ai_{version}.exe",
        DIST_DIR / "android" / f"kk_novel_ai_{version}_arm64-v8a.apk",
    ]
    candidates.extend(sorted(DIST_DIR.glob(f"kk_novel_ai_{version}*.apk")))
    candidates.extend(sorted((DIST_DIR / "android").glob(f"kk_novel_ai_{version}*.apk")) if (DIST_DIR / "android").exists() else [])
    for p in candidates:
        key = str(p.resolve())
        if p.exists() and p.is_file() and key not in seen:
            seen.add(key)
            found.append(p)
    return found


def _ensure_gh_ready(env: dict[str, str]) -> None:
    if shutil.which("gh") is None:
        raise RuntimeError("未找到 gh，请安装 GitHub CLI 并执行 gh auth login（或设置 GH_TOKEN）")
    status = _run_argv(["gh", "auth", "status"], env)
    if status.returncode != 0:
        raise RuntimeError(
            "gh 未登录，无法上传 GitHub Release。"
            "请在本机执行 `gh auth login`（授权 repo），或设置环境变量 GH_TOKEN。"
            "登录后无需重编，执行: python build.py --publish-only"
        )


def _temp_release_path(name: str) -> Path:
    root = Path(os.environ.get("TEMP") or os.environ.get("TMP") or tempfile.gettempdir())
    return root / name


def _truncate_text(text: str, limit: int) -> str:
    if len(text) <= limit:
        return text
    omitted = len(text) - limit
    return text[:limit] + f"\n\n… 已截断，省略约 {omitted} 字符。完整文件列表见 --stat。\n"


def _last_release_base(env: dict[str, str], version: str) -> str:
    desc = _run_argv_capture(["git", "describe", "--tags", "--abbrev=0"], env)
    if desc.returncode == 0:
        tag = desc.stdout.strip()
        if tag and tag not in {f"v{version}", version}:
            return tag
        prev = _run_argv_capture(["git", "describe", "--tags", "--abbrev=0", f"{tag}^"], env)
        if prev.returncode == 0 and prev.stdout.strip():
            return prev.stdout.strip()
        parent = _run_argv_capture(["git", "rev-parse", "--short", f"{tag}^"], env)
        if parent.returncode == 0 and parent.stdout.strip():
            return parent.stdout.strip()
    origin = _run_argv_capture(["git", "rev-parse", "--abbrev-ref", "origin/HEAD"], env)
    if origin.returncode == 0 and origin.stdout.strip():
        ref = origin.stdout.strip()
        return ref if ref.startswith("origin/") else f"origin/{ref}"
    return "HEAD"


def _collect_release_diff(env: dict[str, str], version: str) -> dict[str, Path]:
    base = _last_release_base(env, version)
    print(f"[INFO] 发版 diff 基准: {base}")
    log = _run_argv_capture(["git", "log", "--oneline", f"{base}..HEAD"], env)
    stat = _run_argv_capture(["git", "diff", "--stat", base, "--", ".", *DIFF_EXCLUDES], env)
    diff = _run_argv_capture(["git", "diff", base, "--", ".", *DIFF_EXCLUDES], env)
    paths = {
        "base": base,
        "log": _temp_release_path("kk_novel_ai_release_log.txt"),
        "stat": _temp_release_path("kk_novel_ai_release_stat.txt"),
        "diff": _temp_release_path("kk_novel_ai_release_diff.txt"),
        "prompt": _temp_release_path("kk_novel_ai_release_prompt.md"),
        "commit_msg": _temp_release_path("kk_novel_ai_commit_msg.txt"),
    }
    paths["log"].write_text(log.stdout or "(无新提交)\n", encoding="utf-8")
    paths["stat"].write_text(stat.stdout or "(无 stat)\n", encoding="utf-8")
    paths["diff"].write_text(
        _truncate_text(diff.stdout or "(无 diff)\n", MAX_DIFF_CHARS),
        encoding="utf-8",
    )
    return paths


def _fill_release_prompt(version: str, paths: dict[str, Path]) -> str:
    if not RELEASE_PROMPT_TEMPLATE.exists():
        raise RuntimeError(f"缺少 Cursor 发版提示词: {RELEASE_PROMPT_TEMPLATE}")
    text = RELEASE_PROMPT_TEMPLATE.read_text(encoding="utf-8")
    repl = {
        "{{VERSION}}": version,
        "{{GITHUB_URL}}": GITHUB_URL,
        "{{BASE_REF}}": str(paths["base"]),
        "{{LOG_PATH}}": str(paths["log"]),
        "{{STAT_PATH}}": str(paths["stat"]),
        "{{DIFF_PATH}}": str(paths["diff"]),
        "{{COMMIT_MSG_PATH}}": str(paths["commit_msg"]),
    }
    for k, v in repl.items():
        text = text.replace(k, v)
    paths["prompt"].write_text(text, encoding="utf-8")
    return text


def _prefer_cmd_over_ps1(path: str) -> str:
    p = Path(path)
    if p.suffix.lower() == ".ps1":
        sibling = p.with_suffix(".cmd")
        if sibling.exists():
            return str(sibling)
        alt = p.with_name("agent.cmd")
        if alt.exists():
            return str(alt)
    return path


def _cursor_agent_cmd() -> list[str] | None:
    for name in ("agent.cmd", "agent.exe", "agent", "cursor-agent", "cursor-agent.exe"):
        found = shutil.which(name)
        if found:
            return [_prefer_cmd_over_ps1(found)]
    extras = [
        Path.home() / ".local" / "bin" / "agent.exe",
        Path.home() / ".local" / "bin" / "agent",
        Path(os.environ.get("LOCALAPPDATA", "")) / "cursor-agent" / "agent.cmd",
        Path(os.environ.get("LOCALAPPDATA", "")) / "cursor-agent" / "agent.exe",
        Path(os.environ.get("LOCALAPPDATA", "")) / "cursor-agent" / "cursor-agent.cmd",
    ]
    for p in extras:
        if p and p.exists():
            return [_prefer_cmd_over_ps1(str(p))]
    cursor = shutil.which("cursor") or shutil.which("cursor.cmd")
    if cursor:
        return [cursor, "agent"]
    return None


def _run_cursor_agent_cli(prompt: str, env: dict[str, str]) -> bool:
    cmd = _cursor_agent_cmd()
    if not cmd:
        return False
    argv = [
        *cmd,
        "-p",
        "--force",
        "--trust",
        "--model",
        CURSOR_MODEL,
        "--workspace",
        str(SCRIPT_DIR),
        prompt,
    ]
    print(f"[INFO] 调用 Cursor Agent CLI（模型 {CURSOR_MODEL}）重写 README.md …")
    try:
        r = _run_argv(argv, env, timeout=CURSOR_TIMEOUT_SECS)
    except subprocess.TimeoutExpired as e:
        raise RuntimeError(f"Cursor Agent CLI 超时（{CURSOR_TIMEOUT_SECS}s）") from e
    if r.returncode != 0:
        print(f"[WARN] Cursor Agent CLI 退出码 {r.returncode}，尝试其它方式")
        return False
    return True


def _run_cursor_sdk(prompt: str) -> bool:
    key = os.environ.get("CURSOR_API_KEY", "").strip()
    if not key:
        return False
    try:
        from cursor_sdk import Agent, AgentOptions, LocalAgentOptions
    except ImportError:
        print("[WARN] 未安装 cursor-sdk，跳过 Python SDK 调用")
        return False
    print(f"[INFO] 调用 cursor-sdk（模型 {CURSOR_MODEL}）重写 README.md …")
    try:
        result = Agent.prompt(
            prompt,
            AgentOptions(
                api_key=key,
                model=CURSOR_MODEL,
                local=LocalAgentOptions(cwd=str(SCRIPT_DIR)),
            ),
        )
    except Exception as e:
        print(f"[WARN] cursor-sdk 失败: {e}")
        return False
    status = getattr(result, "status", None)
    if status and str(status).lower() not in {"ok", "success", "completed", "complete"}:
        print(f"[WARN] cursor-sdk 状态: {status}")
        return False
    return True


def _cursor_missing_hint() -> str:
    return (
        "未找到 Cursor Agent CLI，也没有可用的 CURSOR_API_KEY。"
        "请安装 Cursor Agent（PATH 中有 agent），或到 "
        "https://cursor.com/dashboard/integrations 创建 API key 并设置环境变量 "
        "CURSOR_API_KEY；可选 pip install cursor-sdk。"
        "若本次只想打包不重写 README：python build.py --no-cursor-readme"
    )


def _assert_readme_ok(version: str, before_hash: str) -> None:
    if not README_PATH.exists():
        raise RuntimeError("Cursor 跑完后找不到 README.md")
    text = README_PATH.read_text(encoding="utf-8")
    if "github.com/ksasukirk/kk_novel_ai" not in text:
        raise RuntimeError("README.md 缺少 GitHub 仓库地址 https://github.com/ksasukirk/kk_novel_ai")
    after_hash = hashlib.sha256(text.encode("utf-8")).hexdigest()
    if after_hash == before_hash:
        print("[WARN] Cursor 未改动 README.md 内容")
    if version not in text:
        print(f"[WARN] README.md 未出现本版版本号 {version}")


def _commit_subject_prefix(version: str) -> str:
    return f"Release v{version}"


def _join_commit_message(subject: str, body: str) -> str:
    subject = subject.strip()
    body = body.strip()
    if body:
        return f"{subject}\n\n{body}\n"
    return f"{subject}\n"


def _normalize_release_subject(first: str, version: str) -> str:
    """第一行必须是「Release vX: 摘要」，版本号与功能在同一行。"""
    prefix = _commit_subject_prefix(version)
    line = first.strip()
    summary = ""
    if line == prefix or line == f"{prefix}:":
        summary = ""
    elif line.startswith(f"{prefix}:"):
        summary = line[len(prefix) + 1 :].strip()
    elif line.startswith(prefix):
        summary = line[len(prefix) :].strip().lstrip(":：").strip()
    else:
        summary = line
    summary = re.sub(r"\s+", " ", summary).strip(" 。.;；")
    if len(summary) > 72:
        summary = summary[:72].rstrip()
    if summary:
        return f"{prefix}: {summary}"
    return prefix


def _load_commit_message(path: Path, version: str) -> str:
    fallback = f"{_commit_subject_prefix(version)}\n"
    if not path.exists():
        print("[WARN] 未找到 Cursor 写的 commit message，回退 Release 标题")
        return fallback
    raw = path.read_text(encoding="utf-8").replace("\r\n", "\n").strip()
    if not raw:
        return fallback
    lines = raw.splitlines()
    first = lines[0].strip()
    rest_lines = lines[1:]
    while rest_lines and not rest_lines[0].strip():
        rest_lines = rest_lines[1:]
    subject = _normalize_release_subject(first, version)
    prefix = _commit_subject_prefix(version)
    if subject == prefix and rest_lines:
        lifted = rest_lines[0].strip()
        candidate = _normalize_release_subject(f"{prefix}: {lifted}", version)
        if candidate != prefix:
            subject = candidate
    body = "\n".join(rest_lines).strip()
    return _join_commit_message(subject, body)


def _release_notes(commit_msg: str, version: str) -> str:
    lines = commit_msg.strip().splitlines()
    body = "\n".join(lines[1:]).strip()
    if body:
        return body
    first = lines[0].strip() if lines else ""
    prefix = _commit_subject_prefix(version)
    if first.startswith(f"{prefix}:"):
        summary = first[len(prefix) + 1 :].strip()
        if summary:
            return summary
    if README_PATH.exists():
        paras: list[str] = []
        for line in README_PATH.read_text(encoding="utf-8").splitlines():
            s = line.strip()
            if not s or s.startswith("#"):
                if paras:
                    break
                continue
            paras.append(s)
            if len(" ".join(paras)) > 400:
                break
        if paras:
            return " ".join(paras)[:800]
    return f"Kk Novel Ai v{version}"


def _assert_no_secrets_staged(env: dict[str, str]) -> None:
    listed = _run_argv_capture(["git", "diff", "--cached", "--name-only"], env)
    if listed.returncode != 0:
        raise RuntimeError("无法列出暂存文件")
    bad: list[str] = []
    for name in listed.stdout.splitlines():
        n = name.strip().replace("\\", "/")
        if not n:
            continue
        p = Path(n)
        low = n.lower()
        if p.name.lower() in SECRET_NAMES or p.suffix.lower() in SECRET_SUFFIXES or low.endswith(".pem"):
            bad.append(n)
    if bad:
        raise RuntimeError("暂存区含敏感文件，已中止 commit: " + ", ".join(bad))


def _run_cursor_release_docs(version: str, env: dict[str, str]) -> Path:
    """对比往期 diff，调用 Cursor 重写 README.md，返回 commit message 路径。"""
    before = ""
    if README_PATH.exists():
        before = hashlib.sha256(README_PATH.read_bytes()).hexdigest()
    paths = _collect_release_diff(env, version)
    if paths["commit_msg"].exists():
        paths["commit_msg"].unlink()
    prompt = _fill_release_prompt(version, paths)
    short = (
        f"Read and follow every instruction in this file, then stop: {paths['prompt']}"
    )
    cli = _cursor_agent_cmd()
    ok = False
    if cli:
        ok = _run_cursor_agent_cli(short, env)
    if not ok:
        ok = _run_cursor_sdk(prompt)
    if not ok:
        if cli:
            raise RuntimeError(
                "Cursor Agent CLI 未能完成 README 重写。"
                "可设置 CURSOR_API_KEY 并 pip install cursor-sdk，或加 --no-cursor-readme 跳过。"
            )
        raise RuntimeError(_cursor_missing_hint())
    _assert_readme_ok(version, before)
    msg = _load_commit_message(paths["commit_msg"], version)
    paths["commit_msg"].write_text(msg, encoding="utf-8")
    print(f"[OK] Cursor 发版文档完成，commit message: {paths['commit_msg']}")
    return paths["commit_msg"]


def publish_github_release(
    version: str,
    outputs: list[Path],
    env: dict[str, str],
    *,
    cursor_readme: bool = True,
) -> None:
    """全量提交工作区并上传构建产物到 GitHub Release。代码路径: build.py"""
    if shutil.which("git") is None:
        raise RuntimeError("未找到 git，无法提交")
    if shutil.which("gh") is None:
        raise RuntimeError("未找到 gh，请安装 GitHub CLI 并执行 gh auth login（或设置 GH_TOKEN）")

    inside = _run_argv(["git", "rev-parse", "--is-inside-work-tree"], env)
    if inside.returncode != 0:
        raise RuntimeError("当前目录不是 git 仓库，无法发版")

    msg_path = _temp_release_path("kk_novel_ai_commit_msg.txt")
    if cursor_readme:
        msg_path = _run_cursor_release_docs(version, env)
    else:
        print("[INFO] 已跳过 Cursor 重写 README（--no-cursor-readme 或 --publish-only）")
        msg_path.write_text(f"Release v{version}\n", encoding="utf-8")

    add = _run_argv(["git", "add", "-A"], env)
    if add.returncode != 0:
        raise RuntimeError("git add -A 失败")
    _assert_no_secrets_staged(env)

    porcelain = _run_argv_capture(["git", "status", "--short"], env)
    print("[INFO] git status --short")
    sys.stdout.write(porcelain.stdout or "(clean)\n")
    sys.stdout.flush()

    staged_empty = _run_argv(["git", "diff", "--cached", "--quiet"], env)
    if staged_empty.returncode == 0:
        print("[INFO] 暂存区为空，跳过 git commit")
    else:
        commit_msg = _load_commit_message(msg_path, version)
        msg_path.write_text(commit_msg, encoding="utf-8")
        commit = _run_argv(["git", "commit", "-F", str(msg_path)], env)
        if commit.returncode != 0:
            raise RuntimeError("git commit 失败")
        push = _run_argv(["git", "push", "origin", "HEAD"], env)
        if push.returncode != 0:
            raise RuntimeError("git push 失败")
        print(f"[OK] 已提交并推送 Release v{version}")

    assets = [str(p.resolve()) for p in outputs if p.exists() and p.is_file()]
    if not assets:
        raise RuntimeError("没有可上传的构建产物")

    _ensure_gh_ready(env)

    tag = f"v{version}"
    notes = _release_notes(_load_commit_message(msg_path, version), version)
    view = _run_argv(["gh", "release", "view", tag, "-R", GH_REPO], env)
    if view.returncode == 0:
        upload = _run_argv(
            ["gh", "release", "upload", tag, "--clobber", "-R", GH_REPO, *assets],
            env,
        )
        if upload.returncode != 0:
            raise RuntimeError(f"gh release upload {tag} 失败")
        print(f"[OK] 已覆盖上传 GitHub Release {tag}")
        return

    notes_file = _temp_release_path("kk_novel_ai_release_notes.txt")
    notes_file.write_text(notes, encoding="utf-8")
    create = _run_argv(
        [
            "gh",
            "release",
            "create",
            tag,
            "--title",
            tag,
            "--notes-file",
            str(notes_file),
            "-R",
            GH_REPO,
            *assets,
        ],
        env,
    )
    if create.returncode != 0:
        raise RuntimeError(f"gh release create {tag} 失败")
    print(f"[OK] 已创建 GitHub Release {tag}")


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
    parser.add_argument(
        "--no-github-release",
        action="store_true",
        help="构建成功后不提交 git、不重写 README、不上传 GitHub Release",
    )
    parser.add_argument(
        "--github-release",
        action="store_true",
        help="debug 构建也发版（release 默认已发版，无需再加）",
    )
    parser.add_argument(
        "--publish-only",
        action="store_true",
        help="不构建，用当前版本号和 dist/ 已有产物补传 GitHub Release",
    )
    parser.add_argument(
        "--no-cursor-readme",
        action="store_true",
        help="发版时不调用 Cursor 重写 README（仍 git add -A 并上传 Release）",
    )
    args = parser.parse_args()

    if args.publish_only:
        if args.no_github_release:
            raise RuntimeError("--publish-only 与 --no-github-release 不能同时使用")
        version = read_version()
        outputs = collect_dist_outputs(version)
        if not outputs:
            raise RuntimeError(
                f"找不到 dist 产物，无法补发版（需要 dist/kk_novel_ai_{version}.exe）"
            )
        env = os.environ.copy()
        env["BUILD_VERSION"] = version
        print(f"[INFO] 补发 GitHub Release v{version}，产物：")
        for p in outputs:
            print(f"  - {p}")
        publish_github_release(version, outputs, env, cursor_readme=False)
        return

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

    do_release = False
    if args.no_github_release:
        do_release = False
    elif args.configuration == "debug":
        do_release = bool(args.github_release)
    else:
        do_release = True
    if do_release:
        publish_github_release(
            version,
            outputs,
            env,
            cursor_readme=not args.no_cursor_readme,
        )
    else:
        print("[INFO] 已跳过 git 提交与 GitHub Release")


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        sys.exit(130)
    except Exception as e:
        print(f"[ERROR] {e}", file=sys.stderr)
        sys.exit(1)
