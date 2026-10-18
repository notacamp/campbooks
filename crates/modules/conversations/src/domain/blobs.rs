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

    #[test]
    fn snippets_collapse_whitespace() {
        assert_eq!(snippet("  Olá\n\n  Rui,\tveja ", 100), "Olá Rui, veja");
        assert_eq!(snippet("abcdef", 3), "abc");
        assert_eq!(snippet("", 3), "");
    }
}
