//! Reading bodies back from blob storage.

use campbooks_kernel::blobs::{BlobError, BlobRef, BlobStore};
use futures_util::StreamExt as _;

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
