//! Serving attachments: which media types go out as stored, how a file
//! name is made safe for a header, and the `Content-Disposition` built from
//! it. The bytes and their type come from the sender: nothing here trusts
//! them.

use axum::http::HeaderValue;

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

/// Characters that reorder text (bidirectional overrides, embeddings,
/// isolates and marks): `invoice\u{202E}fdp.exe` would read as
/// `invoiceexe.pdf`.
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// `name` made safe to offer as a file name: no path separators, control
/// characters (C0, DEL, C1) or bidirectional controls, no leading dots or
/// surrounding whitespace, at most 150 characters with the extension kept,
/// and `attachment` when nothing is left.
pub fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control() && !is_bidi_control(*c) && !matches!(c, '/' | '\\'))
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
}
