//! Zoho's message metadata (subjects, addresses, summaries) arrives
//! HTML-escaped (`&lt;sam@example.com&gt;`, `&quot;Sam&quot;`, `&#39;`).
//! It is decoded here, once, wherever metadata is read. Bodies and headers
//! come from the raw message and are never touched.

/// `text` with HTML character references decoded: the five named ones Zoho
/// uses, `&nbsp;`, and decimal and hexadecimal references. Anything else is
/// kept as written.
pub fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest
            .find(';')
            .filter(|end| *end <= 10)
            .and_then(|end| reference(&rest[1..end]).map(|c| (c, end)));
        if let Some((c, end)) = decoded {
            out.push(c);
            rest = &rest[end + 1..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

/// The character `name` (between `&` and `;`) stands for.
fn reference(name: &str) -> Option<char> {
    let named = match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    let number = name.strip_prefix('#')?;
    let code = if let Some(hex) = number.strip_prefix(['x', 'X']) {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        number.parse().ok()?
    };
    char::from_u32(code).filter(|c| *c != '\0')
}

/// The addresses of a decoded address list (`"Sam" <sam@example.com>,
/// bea@example.com`), lower-cased. Zoho writes "Not Provided" for none.
pub fn addresses(list: &str) -> Vec<String> {
    list.split(',')
        .filter_map(|part| {
            let part = part.trim();
            let address = match (part.rfind('<'), part.rfind('>')) {
                (Some(open), Some(close)) if open < close => &part[open + 1..close],
                _ => part,
            };
            let address = address.trim().to_lowercase();
            address.contains('@').then_some(address)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_what_zoho_escapes() {
        assert_eq!(
            unescape("&quot;Sam Rivera&quot;&lt;sam@example.com&gt;"),
            "\"Sam Rivera\"<sam@example.com>"
        );
        assert_eq!(unescape("I&#39;m away &amp; back"), "I'm away & back");
        assert_eq!(unescape("Or&#xE7;amento &#231;"), "Orçamento ç");
        assert_eq!(unescape("Q&A; R&D"), "Q&A; R&D");
        assert_eq!(unescape("50% &unknown; &"), "50% &unknown; &");
        assert_eq!(unescape("&#0;"), "&#0;");
    }

    #[test]
    fn reads_address_lists() {
        assert_eq!(
            addresses(&unescape(
                "&quot;Maya&quot;&lt;Maya@Northwind.example&gt;,leo@example.com"
            )),
            ["maya@northwind.example", "leo@example.com"]
        );
        assert!(addresses("Not Provided").is_empty());
    }
}
