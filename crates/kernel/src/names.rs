//! Wire-name rules shared by everything a module registers: jobs, event
//! handlers, events, record types and actions are all `<module>.<name>`.

/// Lower-case snake case: `[a-z][a-z0-9_]*`.
pub(crate) fn is_snake(s: &str) -> bool {
    s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `<snake>.<snake>`.
pub(crate) fn is_dotted(name: &str) -> bool {
    name.split_once('.')
        .is_some_and(|(a, b)| is_snake(a) && is_snake(b))
}

/// `name` is `<owner>.<snake>`, named after the module that registers it.
pub(crate) fn check_owned_name(owner: &str, name: &str) -> Result<(), String> {
    match name.split_once('.') {
        Some((prefix, rest)) if prefix == owner && is_snake(rest) => Ok(()),
        _ => Err(format!(
            "must be `{owner}.<name>` (lower-case snake case), named after its module"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_and_dotted() {
        assert!(is_snake("mail_sync2"));
        assert!(!is_snake("Mail") && !is_snake("2fa") && !is_snake("") && !is_snake("a-b"));
        assert!(is_dotted("mail.sync"));
        assert!(!is_dotted("mail") && !is_dotted("mail.") && !is_dotted("a.b.c"));
    }

    #[test]
    fn owned_names() {
        assert!(check_owned_name("mail", "mail.sync").is_ok());
        assert!(check_owned_name("mail", "calendar.sync").is_err());
        assert!(check_owned_name("mail", "mail.Sync").is_err());
    }
}
