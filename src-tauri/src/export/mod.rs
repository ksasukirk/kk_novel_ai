//! TXT / EPUB / PDF 导出
//! 代码路径: kk_novel_ai/src-tauri/src/export/mod.rs

use crate::error::{AppError, AppResult};
use crate::project;
use krilla::geom::{Point, Size, Transform};
use krilla::image::Image;
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
#[allow(dead_code)]
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

enum ExportSeg {
    Para(String),
    Image { rel: String, caption: String },
}

fn json_str(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn is_illus_val(v: &serde_json::Value) -> bool {
    matches!(
        v.get("type").and_then(|t| t.as_str()),
        Some("illustration") | Some("illus")
    )
}

fn inline_seg(v: &serde_json::Value) -> Option<ExportSeg> {
    if is_illus_val(v) {
        let rel = json_str(v, "rel");
        if rel.trim().is_empty() {
            return None;
        }
        Some(ExportSeg::Image {
            rel,
            caption: json_str(v, "caption"),
        })
    } else {
        let t = json_str(v, "text");
        if t.trim().is_empty() {
            None
        } else {
            Some(ExportSeg::Para(t))
        }
    }
}

fn parent_id_of(n: &serde_json::Value) -> Option<&str> {
    match n.get("parentId") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => v.as_str().filter(|s| !s.is_empty()),
    }
}

fn active_variant_text(node: &serde_json::Value) -> String {
    let active = json_str(node, "activeVariantId");
    let variants = node.get("variants").and_then(|v| v.as_array());
    let Some(list) = variants else {
        return String::new();
    };
    let hit = list.iter().find(|v| json_str(v, "id") == active);
    let v = hit.or_else(|| list.first());
    v.map(|x| json_str(x, "text")).unwrap_or_default()
}

fn walk_active_indices(
    nodes: &[serde_json::Value],
    parent: Option<String>,
    from_variant: Option<String>,
    out: &mut Vec<usize>,
) {
    for (i, n) in nodes.iter().enumerate() {
        let pid = parent_id_of(n).map(|s| s.to_string());
        if pid != parent {
            continue;
        }
        if parent.is_some() {
            let fv = n
                .get("fromVariantId")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            if fv != from_variant {
                continue;
            }
        }
        out.push(i);
        let active = json_str(n, "activeVariantId");
        let nid = json_str(n, "id");
        walk_active_indices(nodes, Some(nid), Some(active), out);
    }
}

fn segments_from_sidecar(blocks: &serde_json::Value) -> Vec<ExportSeg> {
    let mut segs = Vec::new();
    if let Some(arr) = blocks.as_array() {
        for b in arr {
            if b.get("type").and_then(|t| t.as_str()) == Some("gen") {
                let t = json_str(b, "text");
                if !t.trim().is_empty() {
                    segs.push(ExportSeg::Para(t));
                }
            } else if let Some(s) = inline_seg(b) {
                segs.push(s);
            }
        }
        return segs;
    }
    if blocks.get("format").and_then(|v| v.as_u64()).unwrap_or(0) < 2 {
        return segs;
    }
    if let Some(plains) = blocks.get("plains").and_then(|v| v.as_array()) {
        for p in plains {
            if let Some(s) = inline_seg(p) {
                segs.push(s);
            }
        }
    }
    let nodes = blocks
        .get("nodes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut active_idx = Vec::new();
    walk_active_indices(&nodes, None, None, &mut active_idx);
    for i in active_idx {
        let n = &nodes[i];
        let text = active_variant_text(n);
        if !text.trim().is_empty() {
            segs.push(ExportSeg::Para(text));
        }
        if let Some(trail) = n.get("trailingPlains").and_then(|v| v.as_array()) {
            for p in trail {
                if let Some(s) = inline_seg(p) {
                    segs.push(s);
                }
            }
        }
    }
    segs
}

fn chapter_export_segments(root: &Path, chapter_id: &str) -> AppResult<Vec<ExportSeg>> {
    if let Some(blocks) = project::read_genblocks(root, chapter_id) {
        let segs = segments_from_sidecar(&blocks);
        if !segs.is_empty() {
            return Ok(segs);
        }
    }
    let (_, content) = project::read_chapter(root, chapter_id)?;
    let body = strip_kk_gen_markers(content.trim());
    let mut segs = Vec::new();
    for block in body.replace("\r\n", "\n").split("\n\n") {
        let t = block.trim();
        if !t.is_empty() {
            segs.push(ExportSeg::Para(t.to_string()));
        }
    }
    Ok(segs)
}

fn resolve_asset(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.replace('\\', "/").trim().trim_start_matches('/').to_string();
    if rel.is_empty() || rel.contains("..") {
        return None;
    }
    let p = root.join(&rel);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

pub fn export_txt(root: &Path, output: &Path) -> AppResult<()> {
    let opened = project::open_project(root)?;
    let mut buf = String::new();
    buf.push_str(&opened.project.title);
    buf.push_str("\r\n\r\n");
    for ch in &opened.project.chapters {
        buf.push_str("\r\n\r\n");
        buf.push_str(&ch.title);
        buf.push_str("\r\n\r\n");
        let segs = chapter_export_segments(root, &ch.id)?;
        for seg in segs {
            match seg {
                ExportSeg::Para(body) => {
                    for line in body.replace("\r\n", "\n").split('\n') {
                        buf.push_str(line);
                        buf.push_str("\r\n");
                    }
                    buf.push_str("\r\n");
                }
                ExportSeg::Image { caption, .. } => {
                    if caption.trim().is_empty() {
                        buf.push_str("[插图]\r\n\r\n");
                    } else {
                        buf.push_str(&format!("[插图：{}]\r\n\r\n", caption.trim()));
                    }
                }
            }
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

fn image_ext_mime(path: &Path) -> (&'static str, &'static str) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => ("jpg", "image/jpeg"),
        "webp" => ("webp", "image/webp"),
        "gif" => ("gif", "image/gif"),
        _ => ("png", "image/png"),
    }
}

fn segs_to_xhtml_body(
    title: &str,
    segs: &[ExportSeg],
    root: &Path,
    img_files: &mut Vec<(String, Vec<u8>, &'static str)>,
    img_n: &mut usize,
) -> String {
    let mut paras = String::new();
    for seg in segs {
        match seg {
            ExportSeg::Para(body) => {
                for block in body.replace("\r\n", "\n").split("\n\n") {
                    let t = block.trim();
                    if t.is_empty() {
                        continue;
                    }
                    let line = t.replace('\n', "<br/>");
                    paras.push_str(&format!("<p>{}</p>\n", xml_escape(&line)));
                }
            }
            ExportSeg::Image { rel, caption } => {
                let Some(path) = resolve_asset(root, rel) else {
                    if !caption.trim().is_empty() {
                        paras.push_str(&format!(
                            "<p class=\"cap\">{}</p>\n",
                            xml_escape(caption.trim())
                        ));
                    }
                    continue;
                };
                let Ok(bytes) = fs::read(&path) else {
                    continue;
                };
                let (ext, mime) = image_ext_mime(&path);
                *img_n += 1;
                let href = format!("Images/img_{:04}.{}", *img_n, ext);
                img_files.push((href.clone(), bytes, mime));
                paras.push_str(&format!(
                    "<p class=\"illus\"><img src=\"../{href}\" alt=\"{alt}\"/></p>\n",
                    href = href,
                    alt = xml_escape(if caption.trim().is_empty() {
                        "插图"
                    } else {
                        caption.trim()
                    })
                ));
                if !caption.trim().is_empty() {
                    paras.push_str(&format!(
                        "<p class=\"cap\">{}</p>\n",
                        xml_escape(caption.trim())
                    ));
                }
            }
        }
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
p{text-indent:2em;margin:0.6em 0;}
.illus{text-indent:0;text-align:center;margin:1em 0;}
.illus img{max-width:100%;height:auto;}
.cap{text-indent:0;text-align:center;font-size:0.9em;color:#555;margin:0.2em 0 1em;}",
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
    let mut image_files: Vec<(String, Vec<u8>, &'static str)> = Vec::new();
    let mut img_n = 0usize;

    for (i, ch) in opened.project.chapters.iter().enumerate() {
        let id = format!("chap{i}");
        let href = format!("Text/chap_{i:03}.xhtml");
        let segs = chapter_export_segments(root, &ch.id)?;
        let xhtml = segs_to_xhtml_body(&ch.title, &segs, root, &mut image_files, &mut img_n);
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

    for (i, (href, _, mime)) in image_files.iter().enumerate() {
        manifest.push_str(&format!(
            r#"<item id="img{i}" href="{href}" media-type="{mime}"/>"#
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

    for (href, bytes, _) in &image_files {
        zip.start_file(format!("OEBPS/{href}"), opts_deflate)
            .map_err(|e| AppError::msg(e.to_string()))?;
        zip.write_all(bytes)
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

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 {
        return None;
    }
    if bytes[0..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2usize;
    while i + 8 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        if i + 3 >= bytes.len() {
            break;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if matches!(
            marker,
            0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE
                | 0xCF
        ) {
            let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
            if w > 0 && h > 0 {
                return Some((w, h));
            }
            return None;
        }
        if len < 2 {
            break;
        }
        i = i.saturating_add(2).saturating_add(len);
    }
    None
}

fn image_px_size(bytes: &[u8]) -> Option<(u32, u32)> {
    png_size(bytes).or_else(|| jpeg_size(bytes))
}

fn try_krilla_image(bytes: &[u8]) -> Option<Image> {
    let data: krilla::Data = bytes.to_vec().into();
    if bytes.len() >= 8 && bytes[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        Image::from_png(data, false).ok()
    } else if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        Image::from_jpeg(data, false).ok()
    } else {
        let data2: krilla::Data = bytes.to_vec().into();
        Image::from_png(data, false)
            .ok()
            .or_else(|| Image::from_jpeg(data2, false).ok())
    }
}

enum PdfRun {
    Gap(f32),
    Text { text: String, size: f32, height: f32 },
    Image { bytes: Vec<u8>, w: f32, h: f32, caption: String },
}

/// 导出 A4 PDF（嵌入系统中文字体子集）
pub fn export_pdf(root: &Path, output: &Path) -> AppResult<()> {
    let opened = project::open_project(root)?;
    let title = opened.project.title.clone();
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

    let mut runs: Vec<PdfRun> = Vec::new();
    runs.push(PdfRun::Text {
        text: title.clone(),
        size: TITLE_SIZE,
        height: TITLE_SIZE * LINE_GAP,
    });
    runs.push(PdfRun::Gap(BODY_SIZE * 0.6));
    for ch in &opened.project.chapters {
        runs.push(PdfRun::Text {
            text: ch.title.clone(),
            size: HEAD_SIZE,
            height: HEAD_SIZE * LINE_GAP * 1.2,
        });
        runs.push(PdfRun::Gap(BODY_SIZE * 0.4));
        let segs = chapter_export_segments(root, &ch.id)?;
        for seg in segs {
            match seg {
                ExportSeg::Para(body) => {
                    let paras = body.replace("\r\n", "\n");
                    for para in paras.split("\n\n") {
                        let t = para.trim();
                        if t.is_empty() {
                            runs.push(PdfRun::Gap(BODY_SIZE * 0.5));
                            continue;
                        }
                        let flat = t.replace('\n', "");
                        for line in wrap_text_to_width(&flat, BODY_SIZE, max_w) {
                            runs.push(PdfRun::Text {
                                text: line,
                                size: BODY_SIZE,
                                height: BODY_SIZE * LINE_GAP,
                            });
                        }
                        runs.push(PdfRun::Gap(BODY_SIZE * 0.55));
                    }
                }
                ExportSeg::Image { rel, caption } => {
                    if let Some(path) = resolve_asset(root, &rel) {
                        if let Ok(bytes) = fs::read(&path) {
                            let (pw, ph) = image_px_size(&bytes).unwrap_or((1024, 1024));
                            let mut w = max_w;
                            let mut h = w * (ph as f32) / (pw as f32).max(1.0);
                            let max_h = (PAGE_H - MARGIN * 2.0) * 0.45;
                            if h > max_h {
                                h = max_h;
                                w = h * (pw as f32) / (ph as f32).max(1.0);
                            }
                            runs.push(PdfRun::Image {
                                bytes,
                                w,
                                h,
                                caption,
                            });
                            runs.push(PdfRun::Gap(BODY_SIZE * 0.6));
                            continue;
                        }
                    }
                    let label = if caption.trim().is_empty() {
                        "[插图]".into()
                    } else {
                        format!("[插图：{}]", caption.trim())
                    };
                    runs.push(PdfRun::Text {
                        text: label,
                        size: BODY_SIZE,
                        height: BODY_SIZE * LINE_GAP,
                    });
                }
            }
        }
        runs.push(PdfRun::Gap(BODY_SIZE * 0.8));
    }

    let run_height = |r: &PdfRun| match r {
        PdfRun::Gap(h) => *h,
        PdfRun::Text { height, .. } => *height,
        PdfRun::Image { h, caption, .. } => {
            *h + if caption.trim().is_empty() {
                0.0
            } else {
                BODY_SIZE * LINE_GAP
            }
        }
    };

    let mut idx = 0usize;
    while idx < runs.len() {
        let mut page = document.start_page_with(page_settings.clone());
        let mut surface = page.surface();
        let mut y = PAGE_H - MARGIN;
        let page_start = idx;
        while idx < runs.len() {
            let need = run_height(&runs[idx]);
            if y - need < MARGIN {
                break;
            }
            match &runs[idx] {
                PdfRun::Gap(h) => {
                    y -= *h;
                }
                PdfRun::Text { text, size, height } => {
                    if !text.is_empty() {
                        let baseline = y - size * 0.85;
                        surface.draw_text(
                            Point::from_xy(MARGIN, baseline),
                            font.clone(),
                            *size,
                            text,
                            false,
                            TextDirection::Auto,
                        );
                    }
                    y -= *height;
                }
                PdfRun::Image {
                    bytes,
                    w,
                    h,
                    caption,
                } => {
                    if let Some(img) = try_krilla_image(bytes) {
                        if let Some(sz) = Size::from_wh(*w, *h) {
                            let origin_y = y - *h;
                            surface.push_transform(&Transform::from_translate(MARGIN, origin_y));
                            surface.draw_image(img, sz);
                            surface.pop();
                        }
                    }
                    y -= *h;
                    if !caption.trim().is_empty() {
                        let baseline = y - BODY_SIZE * 0.85;
                        surface.draw_text(
                            Point::from_xy(MARGIN, baseline),
                            font.clone(),
                            BODY_SIZE,
                            caption.trim(),
                            false,
                            TextDirection::Auto,
                        );
                        y -= BODY_SIZE * LINE_GAP;
                    }
                }
            }
            idx += 1;
        }
        surface.finish();
        page.finish();
        if idx == page_start {
            idx += 1;
        }
    }

    if runs.is_empty() {
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
