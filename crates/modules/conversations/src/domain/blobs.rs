//! Reading bodies back from blob storage.

use campbooks_kernel::blobs::{BlobError, BlobRef, BlobStore};
use futures_util::StreamExt as _;

use super::plain_text::{self, Quotes};

/// How much of each part [`words`] reads, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Of the text part.
    pub text: usize,
    /// Of the HTML part, read only when the text part has no words.
    pub html: usize,
}

/// A message's words: its text part when that has any, else its HTML part
/// read as text ([`plain_text::text_of`] with `quotes`), else nothing; and
/// whether a part was cut. A text part of blanks only counts as none: mail
/// stored before campbooks-cloud#218 may hold one, made out of HTML the
/// connector's parser could not read.
pub async fn words(
    blobs: &BlobStore,
    workspace: &str,
    (text, html): (Option<&BlobRef>, Option<&BlobRef>),
    budget: Budget,
    quotes: Quotes,
) -> Result<(String, bool), BlobError> {
    if let Some(text) = text {
        let (read, cut) = read_text(blobs, workspace, text, budget.text).await?;
        if !read.trim().is_empty() || html.is_none() {
            return Ok((read, cut));
        }
    }
    match html {
        Some(html) => {
            let (read, cut) = read_text(blobs, workspace, html, budget.html).await?;
            Ok((plain_text::text_of(&read, quotes), cut))
        }
        None => Ok((String::new(), false)),
    }
}

/// `text` cut to at most `max` bytes, at a character boundary.
pub fn cut(mut text: String, max: usize) -> String {
    if text.len() > max {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    text
}

/// The text of `blob` (UTF-8, lossy) up to `max_bytes`, cut at a character
/// boundary, and whether it was cut.
pub async fn read_text(
    blobs: &BlobStore,
    workspace: &str,
    blob: &BlobRef,
    max_bytes: usize,
) -> Result<(String, bool), BlobError> {
    let (bytes, cut) = read_bytes(blobs, workspace, blob, max_bytes).await?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if cut && text.ends_with('\u{fffd}') {
        // The cut fell inside a character.
        text.pop();
    }
    Ok((text, cut))
}

/// The bytes of `blob` up to `max_bytes`, and whether there were more.
pub async fn read_bytes(
    blobs: &BlobStore,
    workspace: &str,
    blob: &BlobRef,
    max_bytes: usize,
) -> Result<(Vec<u8>, bool), BlobError> {
    let mut stream = blobs.get(workspace, blob).await?;
    let mut bytes = Vec::new();
    let mut cut = false;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let room = max_bytes.saturating_sub(bytes.len());
        if chunk.len() > room {
            bytes.extend_from_slice(&chunk[..room]);
            cut = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((bytes, cut))
}

/// The first `chars` characters of `text`, whitespace collapsed.
pub fn snippet(text: &str, chars: usize) -> String {
    text.split_whitespace()
        .flat_map(|word| std::iter::once(' ').chain(word.chars()))
        .skip(1)
        .take(chars)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The words every reader gets (decide, reply context, summaries,
    /// search, the conversation's text): campbooks-cloud#218.
    #[sqlx::test(migrations = "../../../migrations")]
    async fn words_fall_back_to_the_html_when_the_text_has_none(db: sqlx::PgPool) {
        let kit = crate::testkit::Kit::new(db).await;
        let blobs = &kit.state.blobs;
        let workspace = campbooks_kernel::tenancy::WorkspaceId::new().to_string();
        let ws = workspace.as_str();
        let put = |body: &'static str, kind: &'static str| async move {
            blobs.put(ws, body.as_bytes().to_vec(), kind).await.unwrap()
        };
        let html = put(
            "<head><meta charset=utf-8><title>T</title><p>Hi</p><blockquote>Old</blockquote>",
            "text/html; charset=utf-8",
        )
        .await;
        let blank = put("\n\n\n", "text/plain; charset=utf-8").await;
        let text = put("Plain words", "text/plain; charset=utf-8").await;
        let budget = Budget {
            text: 1024,
            html: 1024,
        };
        let read = |parts, quotes| words(blobs, ws, parts, budget, quotes);
        // HTML alone, or beside blanks: its words, quotes kept or dropped.
        for parts in [(None, Some(&html)), (Some(&blank), Some(&html))] {
            let (kept, _) = read(parts, Quotes::Keep).await.unwrap();
            assert!(kept.contains("Hi") && kept.contains("Old") && !kept.contains('T'));
            let (dropped, _) = read(parts, Quotes::Drop).await.unwrap();
            assert!(
                dropped.contains("Hi") && !dropped.contains("Old"),
                "{dropped:?}"
            );
        }
        // A text part with words wins; blanks with nothing else stay blanks.
        assert_eq!(
            read((Some(&text), Some(&html)), Quotes::Keep)
                .await
                .unwrap()
                .0,
            "Plain words"
        );
        assert_eq!(
            read((Some(&blank), None), Quotes::Keep).await.unwrap().0,
            "\n\n\n"
        );
        assert_eq!(read((None, None), Quotes::Keep).await.unwrap().0, "");
        // A cut at a character boundary.
        assert_eq!(cut("éé".to_owned(), 3), "é");
    }

    #[test]
    fn snippets_collapse_whitespace() {
        assert_eq!(snippet("  Olá\n\n  Rui,\tveja ", 100), "Olá Rui, veja");
        assert_eq!(snippet("abcdef", 3), "abc");
        assert_eq!(snippet("", 3), "");
    }
}
