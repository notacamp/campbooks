//! Reading an `Authentication-Results` header (RFC 8601).
//!
//! The header is the receiving provider's, but parts of it echo what the
//! sender chose: the envelope sender in an SPF comment and in
//! `smtp.mailfrom`, which may be a quoted string holding `;`, `(` or `)`.
//! So it is tokenised as RFC 8601 says, never split on characters: comments
//! (nested) are dropped, a quoted string is one atom wherever it appears
//! (inside a comment too, so a `)` in it closes nothing), and only a `;` at
//! the top level ends a result. Anything malformed (an unclosed comment or
//! quoted string, a result without `method=value`) is not read at all: the
//! caller treats a present header it cannot read as a failure (see
//! `Facts::authentication_failed`). The one result section with no method is
//! `none` (RFC 8601 §2.2: nothing was checked), which reads as no results.

/// One method's result, with its own properties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodResult {
    /// `dkim`, `spf`, `dmarc`… (lower-case, version dropped).
    pub method: String,
    /// `pass`, `fail`, `none`… (lower-case).
    pub result: String,
    /// `ptype.property` (lower-case) and its value, in order.
    pub properties: Vec<(String, String)>,
    /// The text of its own top-level comments (the receiver's notes, such as
    /// Gmail's `p=REJECT sp=REJECT dis=NONE` after `dmarc=fail`), quoted
    /// strings and nested comments left out.
    pub comments: Vec<String>,
}

impl MethodResult {
    /// Every value of `property` (`header.from`, …).
    pub fn values<'a>(&'a self, property: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.properties
            .iter()
            .filter(move |(key, _)| key == property)
            .map(|(_, value)| value.as_str())
    }
}

/// The longest `Authentication-Results` value read, in bytes. A real
/// provider's header is a few hundred bytes; a sender can pad the parts it
/// echoes (the envelope sender, comments) to make the tokeniser work, and a
/// padded header is not one a provider wrote. Anything longer is not read.
pub const MAX_LEN: usize = 8 * 1024;

/// Whether `value` is longer than [`MAX_LEN`]: not read, and not to be taken
/// for an absent header (see `Facts::authentication_failed`).
pub fn is_oversized(value: &str) -> bool {
    value.len() > MAX_LEN
}

/// The results of `value` after its authserv-id: empty for `authserv-id; none`
/// (the receiver checked nothing), or `None` when it is malformed, has no
/// result section at all, or is longer than [`MAX_LEN`].
pub fn parse(value: &str) -> Option<Vec<MethodResult>> {
    if is_oversized(value) {
        return None;
    }
    let segments = tokenise(value)?;
    let mut segments = segments.into_iter();
    // The authserv-id (and an optional version) comes first.
    segments
        .next()
        .filter(|segment| !segment.tokens.is_empty())?;
    let sections: Vec<Segment> = segments
        .filter(|segment| !segment.tokens.is_empty())
        .collect();
    if let [only] = sections.as_slice()
        && let [word] = only.tokens.as_slice()
        && word.eq_ignore_ascii_case("none")
    {
        return Some(Vec::new());
    }
    if sections.is_empty() {
        return None;
    }
    sections
        .into_iter()
        .map(|segment| {
            method_result(&glue(&segment.tokens)).map(|result| MethodResult {
                comments: segment.comments,
                ..result
            })
        })
        .collect()
}

/// `tokens` with the whitespace RFC 8601 allows around `=` (and around `/`
/// in the method) taken out: `dkim = pass`, `dkim/1 = pass`,
/// `header.d = x` (a comment before the `=` leaves the same gap). An empty
/// value (`smtp.mailfrom=`) takes nothing from what follows it when that is
/// itself a `property=value` pair.
fn glue(tokens: &[String]) -> Vec<String> {
    let pair = |token: &str| {
        token
            .split_once('=')
            .is_some_and(|(key, _)| key.contains('.'))
    };
    let mut out: Vec<String> = Vec::new();
    let mut join = false;
    for token in tokens {
        let first = out.len() == 1;
        let attach = out.last().is_some()
            && (token.starts_with('=')
                || (first && token.starts_with('/'))
                || (join && !pair(token)));
        if attach {
            if let Some(last) = out.last_mut() {
                last.push_str(token);
            }
        } else {
            out.push(token.clone());
        }
        join = token.ends_with('=') || (out.len() == 1 && token.ends_with('/'));
    }
    out
}

/// One top-level `;`-separated part: its tokens, and its comments' text.
#[derive(Default)]
struct Segment {
    tokens: Vec<String>,
    comments: Vec<String>,
}

/// The header's top-level `;`-separated parts, each as its tokens, with
/// comments set apart (their own top-level text only) and quoted strings
/// kept whole (without their quotes).
fn tokenise(value: &str) -> Option<Vec<Segment>> {
    fn flush(token: &mut String, segments: &mut [Segment]) {
        if !token.is_empty()
            && let Some(last) = segments.last_mut()
        {
            last.tokens.push(std::mem::take(token));
        }
    }
    let mut segments: Vec<Segment> = vec![Segment::default()];
    let mut token = String::new();
    // The comment being read, at depth 1.
    let mut comment = String::new();
    let mut depth = 0_usize;
    let mut quoted = false;
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            if depth == 0 {
                token.push(c);
            }
            escaped = false;
            continue;
        }
        if quoted {
            match c {
                '\\' => escaped = true,
                '"' => quoted = false,
                _ if depth == 0 => token.push(c),
                _ => {}
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '\\' if depth > 0 => escaped = true,
            '(' => {
                flush(&mut token, &mut segments);
                depth += 1;
            }
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0
                    && let Some(last) = segments.last_mut()
                {
                    last.comments
                        .push(std::mem::take(&mut comment).trim().to_owned());
                }
            }
            c if depth == 1 => comment.push(c),
            _ if depth > 0 => {}
            ';' => {
                flush(&mut token, &mut segments);
                segments.push(Segment::default());
            }
            c if c.is_whitespace() => flush(&mut token, &mut segments),
            c => token.push(c),
        }
    }
    if depth > 0 || quoted || escaped {
        return None;
    }
    flush(&mut token, &mut segments);
    Some(segments)
}

/// `method[/version]=result` then `ptype.property=value` pairs (and an
/// optional `reason=…`, ignored).
fn method_result(tokens: &[String]) -> Option<MethodResult> {
    let (method, result) = tokens.first()?.split_once('=')?;
    let method = method.split('/').next()?.trim().to_ascii_lowercase();
    let result = result.trim().to_ascii_lowercase();
    if method.is_empty() || result.is_empty() {
        return None;
    }
    let properties = tokens[1..]
        .iter()
        .filter_map(|token| token.split_once('='))
        .filter(|(key, _)| key.contains('.'))
        .map(|(key, value)| (key.to_ascii_lowercase(), value.to_owned()))
        .collect();
    Some(MethodResult {
        method,
        result,
        properties,
        comments: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn methods(value: &str) -> Vec<(String, String)> {
        parse(value)
            .unwrap()
            .into_iter()
            .map(|r| (r.method, r.result))
            .collect()
    }

    #[test]
    fn reads_a_providers_results() {
        let results = parse(
            "mx.google.com;\
             dkim=pass header.i=@github.com header.s=pf2023 header.b=AbC1;\
             spf=pass (google.com: domain of noreply@github.com designates 192.0.2.10 as \
             permitted sender) smtp.mailfrom=noreply@github.com;\
             dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=github.com",
        )
        .unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[2].method, "dmarc");
        assert_eq!(results[2].result, "pass");
        assert_eq!(
            results[2].values("header.from").collect::<Vec<_>>(),
            ["github.com"]
        );
        assert_eq!(
            results[0].values("header.i").collect::<Vec<_>>(),
            ["@github.com"]
        );
        // Each result keeps its own comments' text.
        assert_eq!(results[2].comments, ["p=REJECT sp=REJECT dis=NONE"]);
        assert!(results[0].comments.is_empty());
    }

    #[test]
    fn a_quoted_envelope_sender_is_one_atom() {
        // The sender's chosen MAIL FROM, echoed in the SPF comment and in
        // smtp.mailfrom: its `;` starts no result.
        let results = methods(
            "mx.google.com; dkim=none; spf=softfail (google.com: domain of transitioning \
             \"a; dmarc=pass header.from=crates.io x\"@evil.example does not designate \
             192.0.2.1 as permitted sender) \
             smtp.mailfrom=\"a; dmarc=pass header.from=crates.io x\"@evil.example; \
             dmarc=fail (p=NONE sp=NONE dis=NONE) header.from=crates.io",
        );
        assert_eq!(
            results,
            [
                ("dkim".into(), "none".into()),
                ("spf".into(), "softfail".into()),
                ("dmarc".into(), "fail".into()),
            ]
        );
    }

    #[test]
    fn a_parenthesis_in_a_quoted_string_closes_no_comment() {
        let results = methods(
            "mx.google.com; spf=softfail (domain of \"a) ; dmarc=pass header.from=crates.io \
             (x\"@evil.example) smtp.mailfrom=x@evil.example; dmarc=fail header.from=crates.io",
        );
        assert_eq!(
            results,
            [
                ("spf".into(), "softfail".into()),
                ("dmarc".into(), "fail".into())
            ]
        );
    }

    #[test]
    fn nested_comments_are_dropped_whole() {
        let results = methods(
            "mx.google.com; spf=pass (outer (inner) ; dmarc=pass header.from=github.com) \
             smtp.mailfrom=x@evil.example",
        );
        assert_eq!(results, [("spf".into(), "pass".into())]);
        let results = parse(
            "mx.google.com; dmarc=pass (p=REJECT (nested; x) dis=NONE) header.from=a.example",
        )
        .unwrap();
        assert_eq!(
            results[0].values("header.from").collect::<Vec<_>>(),
            ["a.example"]
        );
    }

    #[test]
    fn a_header_over_the_length_cap_is_not_read() {
        let head = "mx.google.com; dmarc=pass header.from=github.com; spf=pass (";
        let pad = |len: usize| format!("{head}{}) smtp.mailfrom=a@b.example", "x".repeat(len));
        let fill = MAX_LEN - pad(0).len();
        assert_eq!(pad(fill).len(), MAX_LEN);
        assert_eq!(parse(&pad(fill)).map(|r| r.len()), Some(2));
        assert_eq!(pad(fill + 1).len(), MAX_LEN + 1);
        assert_eq!(parse(&pad(fill + 1)), None);
    }

    #[test]
    fn malformed_headers_are_not_read() {
        for value in [
            "mx.google.com; dmarc=pass (unclosed header.from=github.com",
            "mx.google.com; dmarc=pass header.from=\"github.com",
            "mx.google.com; dmarc=pass) header.from=github.com",
            "mx.google.com; dmarc pass header.from=github.com",
            "; dmarc=pass header.from=github.com",
        ] {
            assert_eq!(parse(value), None, "{value}");
        }
    }

    #[test]
    fn none_is_no_results_and_other_empty_shapes_are_not_read() {
        assert_eq!(parse("mx.google.com; none"), Some(Vec::new()));
        assert_eq!(parse("mx.google.com 1; NONE"), Some(Vec::new()));
        for value in [
            "mx.google.com;",
            "mx.google.com",
            "mx.google.com; none; none",
            "mx.google.com; none dmarc=pass header.from=github.com",
        ] {
            assert_eq!(parse(value), None, "{value}");
        }
    }

    #[test]
    fn rfc_8601_whitespace_and_comments_around_the_equals_sign() {
        let results =
            parse("mx.google.com; dkim (x)=pass header.d = github.com; dkim/1 = fail").unwrap();
        assert_eq!(results[0].result, "pass");
        assert_eq!(
            results[0].values("header.d").collect::<Vec<_>>(),
            ["github.com"]
        );
        assert_eq!(
            (results[1].method.as_str(), results[1].result.as_str()),
            ("dkim", "fail")
        );
    }
}
