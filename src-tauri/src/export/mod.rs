//! TXT / EPUB / PDF 导出
//! 代码路径: kk_novel_ai/src-tauri/src/export/mod.rs

use crate::error::{AppError, AppResult};
use crate::project;
use krilla::geom::Point;
use krilla::page::PageSettings;
use krilla::text::{Font, TextDirection};
use krilla::Document;
use regex::Regex;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// 剥除章节中的 kk-gen 区块外壳，保留中间正文（导出用）
pub fn strip_kk_gen_markers(content: &str) -> String {
    static RE_OPEN: OnceLock<Regex> = OnceLock::new();
    static RE_CLOSE: OnceLock<Regex> = OnceLock::new();
    let open = RE_OPEN.get_or_init(|| {
        Regex::new(r"(?s)<!--\s*kk-gen\b[^>]*-->\s*").expect("kk-gen open re")
    });
    let close = RE_CLOSE.get_or_init(|| {
        Regex::new(r"(?s)\s*<!--\s*/kk-gen\b[^>]*-->").expect("kk-gen close re")
    });
    let s = open.replace_all(content, "");
    close.replace_all(&s, "").into_owned()
}

/// 收集全书纯文本段落（书名 + 各章）
fn collect_export_sections(root: &Path) -> AppResult<(String, Vec<(String, String)>)> {
    let opened = project::open_project(root)?;
    let mut chapters = Vec::new();
    for ch in &opened.project.chapters {
        let (_, content) = project::read_chapter(root, &ch.id)?;
        let body = strip_kk_gen_markers(content.trim()).trim().to_string();
        chapters.push((ch.title.clone(), body));
    }
    Ok((opened.project.title.clone(), chapters))
}

pub fn export_txt(root: &Path, output: &Path) -> AppResult<()> {
    let (title, chapters) = collect_export_sections(root)?;
    let mut buf = String::new();
    buf.push_str(&title);
    buf.push_str("\r\n\r\n");
    for (ch_title, body) in chapters {
        buf.push_str("\r\n\r\n");
        buf.push_str(&ch_title);
        buf.push_str("\r\n\r\n");
        // 统一换行，方便 Windows 记事本
        for line in body.replace("\r\n", "\n").split('\n') {
            buf.push_str(line);
            buf.push_str("\r\n");
        }
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, buf)?;
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn md_to_xhtml_body(title: &str, content: &str) -> String {
    let mut paras = String::new();
    for block in content.replace("\r\n", "\n").split("\n\n") {
        let t = block.trim();
        if t.is_empty() {
            continue;
        }
        let line = t.replace('\n', "<br/>");
        paras.push_str(&format!("<p>{}</p>\n", xml_escape(&line)));
    }
    if paras.is_empty() {
        paras.push_str("<p></p>");
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="zh-CN">
<head>
  <title>{title}</title>
  <link rel="stylesheet" type="text/css" href="../Styles/style.css"/>
</head>
<body>
  <h1>{title}</h1>
  {paras}
</body>
</html>"#,
        title = xml_escape(title),
        paras = paras
    )
}

pub fn export_epub(root: &Path, output: &Path) -> AppResult<()> {
    let opened = project::open_project(root)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(output)
        .map_err(|e| AppError::msg(format!("创建 epub 失败: {e}")))?;
    let mut zip = ZipWriter::new(file);
    let opts_stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let opts_deflate =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file("mimetype", opts_stored)
        .map_err(|e| AppError::msg(e.to_string()))?;
    zip.write_all(b"application/epub+zip")
        .map_err(|e| AppError::msg(e.to_string()))?;

    zip.start_file("META-INF/container.xml", opts_deflate)
        .map_err(|e| AppError::msg(e.to_string()))?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
    )
    .map_err(|e| AppError::msg(e.to_string()))?;

    zip.start_file("OEBPS/Styles/style.css", opts_deflate)
        .map_err(|e| AppError::msg(e.to_string()))?;
    zip.write_all(
        b"body{font-family:serif;line-height:1.7;margin:1em;}
h1{font-size:1.4em;margin:0.8em 0;}
p{text-indent:2em;margin:0.6em 0;}",
    )
    .map_err(|e| AppError::msg(e.to_string()))?;

    let title = &opened.project.title;
    let desc = format!("{} {}", opened.project.genre, opened.project.style);
    let mut manifest = String::from(
        r#"<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
<item id="css" href="Styles/style.css" media-type="text/css"/>"#,
    );
    let mut spine = String::new();
    let mut nav = String::new();
    let mut chapter_files: Vec<(String, String)> = Vec::new();

    for (i, ch) in opened.project.chapters.iter().enumerate() {
        let id = format!("chap{i}");
        let href = format!("Text/chap_{i:03}.xhtml");
        let (_, content) = project::read_chapter(root, &ch.id)?;
        let xhtml = md_to_xhtml_body(&ch.title, &strip_kk_gen_markers(&content));
        chapter_files.push((href.clone(), xhtml));
        manifest.push_str(&format!(
            r#"<item id="{id}" href="{href}" media-type="application/xhtml+xml"/>"#
        ));
        spine.push_str(&format!(r#"<itemref idref="{id}"/>"#));
        nav.push_str(&format!(
            r#"<navPoint id="nav{i}" playOrder="{ord}"><navLabel><text>{t}</text></navLabel><content src="{href}"/></navPoint>"#,
            ord = i + 1,
            t = xml_escape(&ch.title),
            href = href
        ));
    }

    let opf = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>{title}</dc:title>
    <dc:language>zh-CN</dc:language>
    <dc:identifier id="BookId">urn:uuid:{uid}</dc:identifier>
    <dc:description>{desc}</dc:description>
  </metadata>
  <manifest>
    {manifest}
  </manifest>
  <spine toc="ncx">
    {spine}
  </spine>
</package>"#,
        title = xml_escape(title),
        uid = opened.project.id,
        desc = xml_escape(desc.trim()),
        manifest = manifest,
        spine = spine
    );
    zip.start_file("OEBPS/content.opf", opts_deflate)
        .map_err(|e| AppError::msg(e.to_string()))?;
    zip.write_all(opf.as_bytes())
        .map_err(|e| AppError::msg(e.to_string()))?;

    let ncx = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd">
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head>
    <meta name="dtb:uid" content="urn:uuid:{uid}"/>
  </head>
  <docTitle><text>{title}</text></docTitle>
  <navMap>
    {nav}
  </navMap>
</ncx>"#,
        uid = opened.project.id,
        title = xml_escape(title),
        nav = nav
    );
    zip.start_file("OEBPS/toc.ncx", opts_deflate)
        .map_err(|e| AppError::msg(e.to_string()))?;
    zip.write_all(ncx.as_bytes())
        .map_err(|e| AppError::msg(e.to_string()))?;

    for (href, xhtml) in chapter_files {
        zip.start_file(format!("OEBPS/{href}"), opts_deflate)
            .map_err(|e| AppError::msg(e.to_string()))?;
        zip.write_all(xhtml.as_bytes())
            .map_err(|e| AppError::msg(e.to_string()))?;
    }

    zip.finish()
        .map_err(|e| AppError::msg(format!("写入 epub 失败: {e}")))?;
    Ok(())
}

#[cfg(windows)]
fn windows_fonts_dir() -> Option<PathBuf> {
    std::env::var_os("WINDIR").map(|w| PathBuf::from(w).join("Fonts"))
}

/// 查找可用的中文字体（优先 TTF，也试 TTC face 0）
fn load_cjk_font() -> AppResult<Font> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    #[cfg(windows)]
    {
        if let Some(dir) = windows_fonts_dir() {
            for name in [
                "msyh.ttc",
                "msyhbd.ttc",
                "simhei.ttf",
                "simkai.ttf",
                "simfang.ttf",
                "simsun.ttc",
                "SIMSUN.TTC",
                "Deng.ttf",
                "DENG.TTF",
                "NotoSansSC-Regular.otf",
                "SourceHanSansSC-Regular.otf",
            ] {
                candidates.push(dir.join(name));
            }
        }
        if let Some(home) = dirs::home_dir() {
            let local = home
                .join("AppData")
                .join("Local")
                .join("Microsoft")
                .join("Windows")
                .join("Fonts");
            for name in ["msyh.ttc", "NotoSansSC-Regular.otf", "SourceHanSansSC-Regular.otf"] {
                candidates.push(local.join(name));
            }
        }
    }

    #[cfg(not(windows))]
    {
        // Android / 其它：优先应用数据目录与常见系统字体路径
        if let Ok(dir) = crate::paths::app_data_dir() {
            for name in [
                "NotoSansSC-Regular.otf",
                "NotoSansSC-Regular.ttf",
                "SourceHanSansSC-Regular.otf",
            ] {
                candidates.push(dir.join("fonts").join(name));
            }
        }
        for path in [
            "/system/fonts/NotoSansCJK-Regular.ttc",
            "/system/fonts/NotoSansSC-Regular.otf",
            "/system/fonts/DroidSansFallback.ttf",
        ] {
            candidates.push(PathBuf::from(path));
        }
    }

    let mut tried = Vec::new();
    for path in candidates {
        if !path.exists() {
            continue;
        }
        tried.push(path.display().to_string());
        if let Ok(bytes) = fs::read(&path) {
            let max_face = if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ttc"))
                .unwrap_or(false)
            {
                3
            } else {
                0
            };
            for face in 0..=max_face {
                if let Some(font) = Font::new(bytes.clone().into(), face) {
                    return Ok(font);
                }
            }
        }
    }
    Err(AppError::msg(format!(
        "未找到可用的中文字体，无法导出 PDF。{}已尝试：{}",
        if cfg!(target_os = "android") {
            "请把 NotoSansSC-Regular.otf 放到应用 fonts 目录，或改用 TXT/EPUB 导出。"
        } else {
            "请安装微软雅黑/黑体，或把 NotoSansSC 放到系统字体目录。"
        },
        if tried.is_empty() {
            "（无候选文件）".into()
        } else {
            tried.join("；")
        }
    )))
}

fn char_advance(ch: char, font_size: f32) -> f32 {
    if ch.is_ascii() {
        font_size * 0.55
    } else {
        font_size
    }
}

fn wrap_text_to_width(text: &str, font_size: f32, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0.0_f32;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0.0;
            continue;
        }
        let w = char_advance(ch, font_size);
        if !cur.is_empty() && cur_w + w > max_width {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0.0;
        }
        cur.push(ch);
        cur_w += w;
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// 导出 A4 PDF（嵌入系统中文字体子集）
pub fn export_pdf(root: &Path, output: &Path) -> AppResult<()> {
    let (title, chapters) = collect_export_sections(root)?;
    let font = load_cjk_font()?;

    const PAGE_W: f32 = 595.0;
    const PAGE_H: f32 = 842.0;
    const MARGIN: f32 = 48.0;
    const TITLE_SIZE: f32 = 20.0;
    const HEAD_SIZE: f32 = 14.0;
    const BODY_SIZE: f32 = 11.0;
    const LINE_GAP: f32 = 1.45;

    let max_w = PAGE_W - MARGIN * 2.0;
    let mut document = Document::new();

    let page_settings = PageSettings::from_wh(PAGE_W, PAGE_H)
        .ok_or_else(|| AppError::msg("无法创建 PDF 页面尺寸"))?;

    let mut all_lines: Vec<(String, f32, f32)> = Vec::new();
    all_lines.push((title.clone(), TITLE_SIZE, TITLE_SIZE * LINE_GAP));
    all_lines.push((String::new(), BODY_SIZE, BODY_SIZE * 0.6));
    for (ch_title, body) in &chapters {
        all_lines.push((ch_title.clone(), HEAD_SIZE, HEAD_SIZE * LINE_GAP * 1.2));
        all_lines.push((String::new(), BODY_SIZE, BODY_SIZE * 0.4));
        let paras = body.replace("\r\n", "\n");
        for para in paras.split("\n\n") {
            let t = para.trim();
            if t.is_empty() {
                all_lines.push((String::new(), BODY_SIZE, BODY_SIZE * 0.5));
                continue;
            }
            let flat = t.replace('\n', "");
            for line in wrap_text_to_width(&flat, BODY_SIZE, max_w) {
                all_lines.push((line, BODY_SIZE, BODY_SIZE * LINE_GAP));
            }
            all_lines.push((String::new(), BODY_SIZE, BODY_SIZE * 0.55));
        }
        all_lines.push((String::new(), BODY_SIZE, BODY_SIZE * 0.8));
    }

    let mut idx = 0usize;
    while idx < all_lines.len() {
        let mut page = document.start_page_with(page_settings.clone());
        let mut surface = page.surface();
        let mut y = PAGE_H - MARGIN;
        let page_start = idx;
        while idx < all_lines.len() {
            let (ref text, size, height) = all_lines[idx];
            if y - height < MARGIN {
                break;
            }
            if !text.is_empty() {
                let baseline = y - size * 0.85;
                surface.draw_text(
                    Point::from_xy(MARGIN, baseline),
                    font.clone(),
                    size,
                    text,
                    false,
                    TextDirection::Auto,
                );
            }
            y -= height;
            idx += 1;
        }
        surface.finish();
        page.finish();
        if idx == page_start {
            idx += 1;
        }
    }

    if all_lines.is_empty() {
        let mut page = document.start_page_with(page_settings);
        let mut surface = page.surface();
        surface.draw_text(
            Point::from_xy(MARGIN, PAGE_H - MARGIN - TITLE_SIZE),
            font,
            TITLE_SIZE,
            &title,
            false,
            TextDirection::Auto,
        );
        surface.finish();
        page.finish();
    }

    let pdf = document
        .finish()
        .map_err(|e| AppError::msg(format!("生成 PDF 失败: {e:?}")))?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, pdf).map_err(|e| AppError::msg(format!("写入 PDF 失败: {e}")))?;
    Ok(())
}
