//! Serving attachments: which media types go out as stored, how a file
//! name is made safe for a header, and the `Content-Disposition` built from
//! it. The bytes and their type come from the sender: nothing here trusts
//! them.

use axum::http::HeaderValue;
use campbooks_kernel::text::is_displayable;

/// What every type off the safe list is served as.
pub const OCTET_STREAM: &str = "application/octet-stream";

/// Raster images: decoded by an `<img>` and nothing else, so they can be
/// previewed and shown inline.
pub const RASTER_IMAGES: [&str; 6] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/avif",
    "image/bmp",
];

/// Types served as stored: a browser neither runs them nor renders them as
/// a document of the app. HTML, XHTML, SVG, XML, scripts and anything
/// unknown are not here, and go out as [`OCTET_STREAM`].
const SAFE_TYPES: [&str; 25] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/avif",
    "image/bmp",
    "application/pdf",
    "text/plain",
    "text/csv",
    "text/calendar",
    "application/zip",
    "application/gzip",
    "application/msword",
    "application/vnd.ms-excel",
    "application/vnd.ms-powerpoint",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/vnd.oasis.opendocument.text",
    "application/vnd.oasis.opendocument.spreadsheet",
    "audio/mpeg",
    "audio/ogg",
    "audio/wav",
    "video/mp4",
    "video/webm",
];

/// Longest file name served, in characters.
const MAX_NAME_CHARS: usize = 150;

/// The type `stored` is served as: its essence (before any `;`, trimmed and
/// lower-cased) when that is on the safe list, else [`OCTET_STREAM`].
pub fn served_type(stored: &str) -> &'static str {
    let essence = essence(stored);
    SAFE_TYPES
        .iter()
        .find(|safe| **safe == essence)
        .copied()
        .unwrap_or(OCTET_STREAM)
}

/// Whether `stored` is a raster image.
pub fn is_raster(stored: &str) -> bool {
    let essence = essence(stored);
    RASTER_IMAGES.contains(&essence.as_str())
}

/// Whether an attachment is an image the message's body shows (sent inline
/// with a `Content-ID`, a raster type), not a file of its own: a signature
/// logo. The reader lists it in the body, and the action catalogue never
/// counts it as an attachment to open.
pub fn in_body(inline: bool, has_content_id: bool, stored: &str) -> bool {
    inline && has_content_id && is_raster(stored)
}

/// Whether a served type may be shown inline (a raster image or a PDF).
pub fn may_be_inline(served: &str) -> bool {
    served == "application/pdf" || RASTER_IMAGES.contains(&served)
}

/// The raster image or PDF `bytes` are, by their first bytes (magic
/// numbers), whatever type they were sent as; `None` for anything else.
/// A cheap second check on top of the safe list: a part sent as
/// `image/png` that holds markup is served as an opaque download.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let at = |offset: usize, magic: &[u8]| bytes.get(offset..offset + magic.len()) == Some(magic);
    if at(0, b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if at(0, b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if at(0, b"GIF87a") || at(0, b"GIF89a") {
        Some("image/gif")
    } else if at(0, b"RIFF") && at(8, b"WEBP") {
        Some("image/webp")
    } else if at(4, b"ftypavif") || at(4, b"ftypavis") {
        Some("image/avif")
    } else if at(0, b"BM") && bytes.len() >= 26 {
        Some("image/bmp")
    } else if at(0, b"%PDF-") {
        Some("application/pdf")
    } else {
        None
    }
}

fn essence(content_type: &str) -> String {
    content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

/// `name` made safe to offer as a file name: no path separators, control
/// characters (C0, DEL, C1), bidirectional controls or invisible characters,
/// no leading dots or
/// surrounding whitespace, at most 150 characters with the extension kept,
/// and `attachment` when nothing is left.
pub fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| is_displayable(*c) && !matches!(c, '/' | '\\'))
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    if trimmed.is_empty() {
        return "attachment".to_owned();
    }
    if trimmed.chars().count() <= MAX_NAME_CHARS {
        return trimmed.to_owned();
    }
    // Keep a short extension: cut the stem.
    let extension = trimmed
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty() && ext.chars().count() <= 10);
    match extension {
        Some(ext) => {
            let keep = MAX_NAME_CHARS - ext.chars().count() - 1;
            let stem: String = trimmed.chars().take(keep).collect();
            format!("{}.{ext}", stem.trim_end())
        }
        None => trimmed.chars().take(MAX_NAME_CHARS).collect(),
    }
}

/// Characters RFC 8187 lets through unencoded in `filename*` (`attr-char`).
fn is_attr_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#' | b'$' | b'&' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~'
        )
}

/// `Content-Disposition` for `name` (already [`safe_filename`]): `inline` or
/// `attachment`, an ASCII `filename` for old clients and the exact name as
/// UTF-8 in `filename*` (RFC 6266, RFC 8187).
pub fn content_disposition(inline: bool, name: &str) -> HeaderValue {
    let fallback: String = name
        .chars()
        .map(|c| {
            if (c.is_ascii_graphic() && !matches!(c, '"' | '\\' | '%' | ';')) || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = name
        .bytes()
        .map(|byte| {
            if is_attr_char(byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    let kind = if inline { "inline" } else { "attachment" };
    let value = format!("{kind}; filename=\"{fallback}\"; filename*=UTF-8''{encoded}");
    // Visible ASCII only by construction.
    HeaderValue::from_str(&value).unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

/// Most files a draft may carry.
pub const MAX_FILES: usize = 20;
/// Longest stored file name, in bytes.
pub const MAX_NAME_BYTES: usize = 255;
/// Longest extension kept when a name is cut.
const MAX_KEPT_EXTENSION: usize = 20;

/// Extensions mail providers refuse to carry (Gmail's list), lower-case.
pub const BLOCKED_EXTENSIONS: [&str; 53] = [
    "ade",
    "adp",
    "apk",
    "appx",
    "appxbundle",
    "bat",
    "cab",
    "chm",
    "cmd",
    "com",
    "cpl",
    "diagcab",
    "diagcfg",
    "diagpack",
    "dll",
    "dmg",
    "ex",
    "ex_",
    "exe",
    "hta",
    "img",
    "ins",
    "iso",
    "isp",
    "jar",
    "jnlp",
    "js",
    "jse",
    "lib",
    "lnk",
    "mde",
    "msc",
    "msi",
    "msix",
    "msixbundle",
    "msp",
    "mst",
    "nsh",
    "pif",
    "ps1",
    "scr",
    "sct",
    "shb",
    "sys",
    "vb",
    "vbe",
    "vbs",
    "vhd",
    "vxd",
    "wsc",
    "wsf",
    "wsh",
    "xll",
];

/// An uploaded file's name as stored: the part after the last `/` or `\`,
/// without control or invisible characters or quotes, spaces and dots
/// trimmed at both ends, at most [`MAX_NAME_BYTES`] bytes (cut on a
/// character boundary, keeping a short extension). `None` when nothing is
/// left.
pub fn clean_upload_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let kept: String = base
        .chars()
        .filter(|c| is_displayable(*c) && *c != '"')
        .collect();
    let trimmed = kept.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.len() <= MAX_NAME_BYTES {
        return Some(trimmed.to_owned());
    }
    let (stem, extension) = match trimmed.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= MAX_KEPT_EXTENSION => {
            (stem, format!(".{ext}"))
        }
        _ => (trimmed, String::new()),
    };
    let mut end = MAX_NAME_BYTES - extension.len();
    while !stem.is_char_boundary(end) {
        end -= 1;
    }
    let stem = stem[..end].trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    if stem.is_empty() {
        return None;
    }
    Some(format!("{stem}{extension}"))
}

/// Whether `name` ends in an extension mail providers block
/// ([`BLOCKED_EXTENSIONS`], case-insensitive).
pub fn is_blocked(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        let extension = extension.to_ascii_lowercase();
        BLOCKED_EXTENSIONS.contains(&extension.as_str())
    })
}

/// The media type an upload is stored with: the request's, without
/// parameters and lower-cased, when it is a well-formed `type/subtype`;
/// else [`OCTET_STREAM`]. Kept for the message, never trusted.
pub fn upload_type(sent: Option<&str>) -> String {
    let token = |part: &str| {
        !part.is_empty()
            && part.len() <= 127
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
    };
    let essence = essence(sent.unwrap_or_default());
    match essence.split_once('/') {
        Some((kind, subtype)) if token(kind) && token(subtype) => essence,
        _ => OCTET_STREAM.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_safe_types_are_served_as_stored() {
        assert_eq!(served_type("application/pdf"), "application/pdf");
        assert_eq!(served_type("IMAGE/PNG; name=a.png"), "image/png");
        assert_eq!(served_type(" text/plain ; charset=utf-8"), "text/plain");
        for unsafe_type in [
            "text/html",
            "TEXT/HTML; charset=utf-8",
            "application/xhtml+xml",
            "image/svg+xml",
            "text/xml",
            "application/xml",
            "application/javascript",
            "text/javascript",
            "application/x-msdownload",
            "multipart/related",
            "",
            "nonsense",
        ] {
            assert_eq!(served_type(unsafe_type), OCTET_STREAM, "{unsafe_type}");
        }
    }

    #[test]
    fn sniffing_names_raster_images_and_pdfs_by_their_magic_bytes() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
        assert_eq!(sniff(b"\xff\xd8\xff\xe0"), Some("image/jpeg"));
        assert_eq!(sniff(b"GIF89a.."), Some("image/gif"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"\0\0\0\x1cftypavif"), Some("image/avif"));
        assert_eq!(sniff(b"%PDF-1.7"), Some("application/pdf"));
        for other in [
            &b"<html><script>alert(1)</script>"[..],
            b"<svg xmlns='http://www.w3.org/2000/svg'/>",
            b"\xff\xd8",
            b"BM",
            b"",
        ] {
            assert_eq!(sniff(other), None, "{other:?}");
        }
    }

    #[test]
    fn only_raster_images_and_pdfs_may_be_inline() {
        assert!(may_be_inline("image/jpeg"));
        assert!(may_be_inline("application/pdf"));
        assert!(!may_be_inline("text/plain"));
        assert!(!may_be_inline(OCTET_STREAM));
        assert!(is_raster("image/png"));
        assert!(!is_raster("image/svg+xml"));
    }

    #[test]
    fn malicious_names_lose_separators_controls_and_bidi_overrides() {
        assert_eq!(
            safe_filename("../../etc/pass\r\nwd\u{202E}gpj.exe"),
            "etcpasswdgpj.exe"
        );
        assert_eq!(safe_filename("..\\..\\boot.ini"), "boot.ini");
        assert_eq!(safe_filename("\u{0}\u{7}\u{85}  "), "attachment");
        assert_eq!(safe_filename(".htaccess"), "htaccess");
        assert_eq!(safe_filename("orçamento 2026.pdf"), "orçamento 2026.pdf");
        assert_eq!(safe_filename("a\u{2066}b\u{200F}c"), "abc");
        let long = format!("{}.pdf", "x".repeat(400));
        let cut = safe_filename(&long);
        assert_eq!(cut.chars().count(), 150);
        assert_eq!(
            std::path::Path::new(&cut).extension(),
            Some(std::ffi::OsStr::new("pdf"))
        );
    }

    #[test]
    fn the_disposition_is_one_valid_header_with_both_names() {
        let value = content_disposition(false, &safe_filename("Fatura \"março\"; x=1.pdf"));
        let text = value.to_str().unwrap();
        assert_eq!(
            text,
            "attachment; filename=\"Fatura _mar_o__ x=1.pdf\"; filename*=UTF-8''Fatura%20%22mar%C3%A7o%22%3B%20x%3D1.pdf"
        );
        let inline = content_disposition(true, "a.pdf");
        assert_eq!(
            inline.to_str().unwrap(),
            "inline; filename=\"a.pdf\"; filename*=UTF-8''a.pdf"
        );
    }

    #[test]
    fn upload_names_are_cleaned() {
        assert_eq!(
            clean_upload_name("../../etc/passwd").as_deref(),
            Some("passwd")
        );
        assert_eq!(
            clean_upload_name(r"C:\Users\sam\lease 4b.pdf").as_deref(),
            Some("lease 4b.pdf")
        );
        assert_eq!(
            clean_upload_name("  \"a\u{0}b\u{202e}.txt. ").as_deref(),
            Some("ab.txt")
        );
        assert_eq!(clean_upload_name("..."), None);
        assert_eq!(clean_upload_name("dir/"), None);
        assert_eq!(clean_upload_name(""), None);
        let long = format!("{}.pdf", "é".repeat(300));
        let cut = clean_upload_name(&long).unwrap();
        assert!(cut.len() <= MAX_NAME_BYTES, "{}", cut.len());
        assert_eq!(cut.rsplit('.').next(), Some("pdf"), "{cut}");
        assert!(clean_upload_name(&"a".repeat(400)).unwrap().len() <= MAX_NAME_BYTES);
    }

    #[test]
    fn blocked_extensions_ignore_case() {
        for name in ["setup.EXE", "run.Bat", "x.js", "a.b.scr", "p.ps1", "d.ex_"] {
            assert!(is_blocked(name), "{name}");
        }
        for name in [
            "lease.pdf",
            "exe",
            "notes.exe.txt",
            "a.json",
            "archive.zip",
            ".exe",
        ] {
            assert_eq!(is_blocked(name), name == ".exe", "{name}");
        }
        let mut sorted = BLOCKED_EXTENSIONS;
        sorted.sort_unstable();
        assert_eq!(
            sorted, BLOCKED_EXTENSIONS,
            "keep the list sorted, no duplicates"
        );
        assert!(sorted.windows(2).all(|w| w[0] != w[1]));
    }

    #[test]
    fn upload_types_are_checked_not_trusted() {
        assert_eq!(upload_type(Some("Application/PDF")), "application/pdf");
        assert_eq!(upload_type(Some("text/plain; charset=utf-8")), "text/plain");
        for bad in ["", "pdf", "a/b/c", "text/ html", "<script>/x", "/x", "x/"] {
            assert_eq!(upload_type(Some(bad)), OCTET_STREAM, "{bad}");
        }
        assert_eq!(upload_type(None), OCTET_STREAM);
    }
}
