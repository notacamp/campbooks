//! Strict CBOR for what authenticators send.
//!
//! - **Strict** (the attestation object, extensions): one well-formed data
//!   item, definite lengths, every head in its shortest form, and no map with
//!   the same key twice. Two parsers can then never read the same bytes two
//!   ways.
//! - **CTAP2 canonical** (the credential public key, which WebAuthn L3
//!   §6.5.1.1 requires in that form): strict, and every map's keys sorted by
//!   the length of their encoding, then bytewise (CTAP2 §8, "CTAP2
//!   canonical CBOR encoding form").
//!
//! Both are checked by decoding and re-encoding: `ciborium` writes definite
//! lengths and shortest heads, so any difference from the input is an
//! indefinite length or a longer head than needed.

use ciborium::Value;

use super::{Refusal, refuse};

/// How deep a value may nest (authenticator data is shallow).
const MAX_DEPTH: usize = 16;

fn encode(value: &Value) -> Result<Vec<u8>, Refusal> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(value, &mut bytes).map_err(|_| Refusal("cbor_not_encodable"))?;
    Ok(bytes)
}

/// Whether `value` has a map with a key twice (or unsorted keys, when
/// `sorted`), at any depth.
fn check_maps(value: &Value, sorted: bool, depth: usize) -> Result<(), Refusal> {
    if depth > MAX_DEPTH {
        return refuse("cbor_too_deep");
    }
    match value {
        Value::Map(entries) => {
            let mut previous: Option<Vec<u8>> = None;
            let mut seen: Vec<Vec<u8>> = Vec::with_capacity(entries.len());
            for (key, item) in entries {
                let encoded = encode(key)?;
                if seen.contains(&encoded) {
                    return refuse("cbor_duplicate_key");
                }
                if sorted
                    && let Some(previous) = &previous
                    && (previous.len(), previous.as_slice()) >= (encoded.len(), encoded.as_slice())
                {
                    return refuse("cbor_not_canonical");
                }
                previous = Some(encoded.clone());
                seen.push(encoded);
                check_maps(key, sorted, depth + 1)?;
                check_maps(item, sorted, depth + 1)?;
            }
            Ok(())
        }
        Value::Array(items) => items
            .iter()
            .try_for_each(|item| check_maps(item, sorted, depth + 1)),
        Value::Tag(_, inner) => check_maps(inner, sorted, depth + 1),
        _ => Ok(()),
    }
}

/// Decode the data item at the front of `bytes`, strictly: the value and
/// how many bytes it took.
fn item(bytes: &[u8], sorted: bool) -> Result<(Value, usize), Refusal> {
    let mut rest = bytes;
    let value: Value =
        ciborium::de::from_reader(&mut rest).map_err(|_| Refusal("cbor_malformed"))?;
    let len = bytes.len() - rest.len();
    if encode(&value)? != bytes[..len] {
        return refuse("cbor_not_shortest_or_definite");
    }
    check_maps(&value, sorted, 0)?;
    Ok((value, len))
}

/// The strict data item at the front of `bytes`, and its length.
pub fn strict_item(bytes: &[u8]) -> Result<(Value, usize), Refusal> {
    item(bytes, false)
}

/// The CTAP2-canonical data item at the front of `bytes`, and its length.
pub fn canonical_item(bytes: &[u8]) -> Result<(Value, usize), Refusal> {
    item(bytes, true)
}

/// `bytes` as exactly one strict data item.
pub fn strict(bytes: &[u8]) -> Result<Value, Refusal> {
    let (value, len) = strict_item(bytes)?;
    if len != bytes.len() {
        return refuse("cbor_trailing_bytes");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_items_parse_and_say_where_they_end() {
        // {1: 2} then a trailing byte.
        let (value, len) = strict_item(&[0xa1, 0x01, 0x02, 0xff]).unwrap();
        assert_eq!(len, 3);
        assert!(value.is_map());
        assert_eq!(
            strict(&[0xa1, 0x01, 0x02, 0x00]),
            refuse("cbor_trailing_bytes")
        );
        assert_eq!(strict(&[0xff]), refuse("cbor_malformed"));
    }

    #[test]
    fn duplicate_keys_are_refused_at_any_depth() {
        // {1: 2, 1: 3}
        assert_eq!(
            strict(&[0xa2, 0x01, 0x02, 0x01, 0x03]),
            refuse("cbor_duplicate_key")
        );
        // {"a": {1: 2, 1: 3}}
        assert_eq!(
            strict(&[0xa1, 0x61, b'a', 0xa2, 0x01, 0x02, 0x01, 0x03]),
            refuse("cbor_duplicate_key")
        );
    }

    #[test]
    fn indefinite_lengths_and_long_heads_are_refused() {
        // An indefinite-length map {1: 2}.
        assert_eq!(
            strict(&[0xbf, 0x01, 0x02, 0xff]),
            refuse("cbor_not_shortest_or_definite")
        );
        // An indefinite-length byte string of one chunk.
        assert_eq!(
            strict(&[0x5f, 0x41, 0x00, 0xff]),
            refuse("cbor_not_shortest_or_definite")
        );
        // 1 encoded in two bytes.
        assert_eq!(
            strict(&[0x18, 0x01]),
            refuse("cbor_not_shortest_or_definite")
        );
    }

    #[test]
    fn canonical_maps_have_sorted_keys() {
        // {1: 0, 3: 0, -1: 0} is CTAP2 canonical (0x01 < 0x03 < 0x20).
        assert!(canonical_item(&[0xa3, 0x01, 0x00, 0x03, 0x00, 0x20, 0x00]).is_ok());
        // {3: 0, 1: 0} is not.
        assert_eq!(
            canonical_item(&[0xa2, 0x03, 0x00, 0x01, 0x00]).map(|_| ()),
            refuse("cbor_not_canonical")
        );
        // Shorter encodings first: {"aa": 0, 1: 0} is not.
        assert_eq!(
            canonical_item(&[0xa2, 0x62, b'a', b'a', 0x00, 0x01, 0x00]).map(|_| ()),
            refuse("cbor_not_canonical")
        );
        // Strict alone allows any order.
        assert!(strict(&[0xa2, 0x03, 0x00, 0x01, 0x00]).is_ok());
    }
}
