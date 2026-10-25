//! Where a page sits among its siblings: a fractional-indexing key
//! (the scheme of the `fractional-indexing` package the client uses), so
//! moving one page rewrites only that page. Keys sort as plain strings.
//!
//! The client makes a key between two siblings; the server checks every key
//! it is given, and makes the key after the last sibling when none is given.

use campbooks_kernel::{Error, Result, tenancy::UserId};
use sqlx::PgConnection;

use super::{PageId, Place};

const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// The smallest integer part, which no key may be.
const SMALLEST_INTEGER: &str = "A00000000000000000000000000";

/// The longest key kept.
pub const MAX_LEN: usize = 64;

/// The length of the integer part a key starting with `head` has.
fn integer_len(head: u8) -> Option<usize> {
    match head {
        b'a'..=b'z' => Some(usize::from(head - b'a') + 2),
        b'A'..=b'Z' => Some(usize::from(b'Z' - head) + 2),
        _ => None,
    }
}

/// The integer part of `key`.
fn integer_part(key: &str) -> Option<&str> {
    let len = integer_len(*key.as_bytes().first()?)?;
    key.get(..len)
}

/// Whether `key` is a key the client's library makes or accepts.
pub fn valid(key: &str) -> bool {
    if key.is_empty() || key.len() > MAX_LEN || !key.bytes().all(|b| DIGITS.contains(&b)) {
        return false;
    }
    if key == SMALLEST_INTEGER {
        return false;
    }
    let Some(integer) = integer_part(key) else {
        return false;
    };
    !key[integer.len()..].ends_with('0')
}

/// `key` checked, as a request error when it is not one.
pub fn check(key: &str) -> Result<()> {
    if valid(key) {
        Ok(())
    } else {
        Err(Error::BadRequest(
            crate::MESSAGES.reason("invalid_position"),
        ))
    }
}

/// The next integer after `integer`, or none at the end of the range.
fn increment(integer: &str) -> Option<String> {
    let mut bytes = integer.as_bytes().to_vec();
    let head = bytes.remove(0);
    let mut carry = true;
    for digit in bytes.iter_mut().rev() {
        if !carry {
            break;
        }
        let at = DIGITS.iter().position(|d| d == digit)? + 1;
        if at == DIGITS.len() {
            *digit = DIGITS[0];
        } else {
            *digit = DIGITS[at];
            carry = false;
        }
    }
    if carry {
        if head == b'Z' {
            return Some(format!("a{}", char::from(DIGITS[0])));
        }
        if head == b'z' {
            return None;
        }
        let next = head + 1;
        if next > b'a' {
            bytes.push(DIGITS[0]);
        } else {
            bytes.pop();
        }
        bytes.insert(0, next);
    } else {
        bytes.insert(0, head);
    }
    String::from_utf8(bytes).ok()
}

/// A key after `last` (the last sibling's), or the first key when there is
/// no sibling. Always valid and at most [`MAX_LEN`] long.
pub fn after(last: Option<&str>) -> String {
    let Some(last) = last.filter(|key| valid(key)) else {
        return format!("a{}", char::from(DIGITS[0]));
    };
    integer_part(last)
        .and_then(increment)
        .unwrap_or_else(|| format!("{}V", &last[..last.len().min(MAX_LEN - 1)]))
}

/// The key after the last sibling's, among the pages under `parent` (or at
/// the top level of `place`; a private place holds each member's own).
pub async fn next(
    conn: &mut PgConnection,
    parent: Option<PageId>,
    place: Place,
    member: UserId,
) -> Result<String> {
    let last: Option<String> = sqlx::query_scalar(
        "SELECT max(position) FROM pages.pages
          WHERE parent_id IS NOT DISTINCT FROM $1::uuid AND place = $2
            AND ($1::uuid IS NOT NULL OR place = 'workspace' OR created_by = $3)",
    )
    .bind(parent)
    .bind(place.as_str())
    .bind(member)
    .fetch_one(conn)
    .await?;
    Ok(after(last.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_key_and_the_keys_after() {
        assert_eq!(after(None), "a0");
        assert_eq!(after(Some("a0")), "a1");
        assert_eq!(after(Some("a9")), "aA");
        assert_eq!(after(Some("az")), "b00");
        assert_eq!(after(Some("Zz")), "a0");
        // A key with a fraction moves on to the next integer.
        assert_eq!(after(Some("a0V")), "a1");
        // A key that is not one starts over.
        assert_eq!(after(Some("!")), "a0");
    }

    #[test]
    fn keys_after_always_sort_after() {
        let mut key = after(None);
        for _ in 0..200 {
            let next = after(Some(&key));
            assert!(next > key, "{next} after {key}");
            assert!(valid(&next), "{next}");
            key = next;
        }
    }

    #[test]
    fn what_the_library_makes_is_valid() {
        for key in ["a0", "a1", "a0V", "Zz", "b00", "a0G", "Zy"] {
            assert!(valid(key), "{key}");
        }
        for key in [
            "",
            "0",
            "a",
            "a0 ",
            "a10",
            "A00000000000000000000000000",
            "a0\u{e9}",
            &"a".repeat(65),
        ] {
            assert!(!valid(key), "{key}");
        }
    }
}
