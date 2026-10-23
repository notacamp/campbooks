//! The words of a search typed to find people (`list_senders`,
//! `list_correspondents`), ready for `LIKE ... ESCAPE '\'`.

/// The words of `q`, split on spaces and commas, at most `max`, with the
/// `LIKE` wildcards (`%`, `_`) and the escape (`\`) escaped, so each matches
/// itself.
pub fn like_words(q: &str, max: usize) -> Vec<String> {
    q.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty())
        .take(max)
        .map(|word| {
            word.chars().fold(String::new(), |mut out, c| {
                if matches!(c, '\\' | '%' | '_') {
                    out.push('\\');
                }
                out.push(c);
                out
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::like_words;

    #[test]
    fn wildcards_are_escaped() {
        assert_eq!(like_words("50%_off a\\b", 5), vec!["50\\%\\_off", "a\\\\b"]);
        assert_eq!(
            like_words("a, b  c d e f", 5),
            vec!["a", "b", "c", "d", "e"]
        );
        assert!(like_words(" , ", 5).is_empty());
    }
}
