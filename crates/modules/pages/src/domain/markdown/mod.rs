//! Markdown in and out of a page's body (campbooks-cloud#364).
//!
//! **In** ([`parse`]): the Markdown that `pages.append` and `pages.create`
//! take is read on the server into the neutral blocks of
//! `notion::model` (headings, paragraphs, lists, to-dos, quotes, code and
//! dividers, with bold, italic, strike, code and links), and those are
//! written into the body with the same block builder Notion's changes use.
//! The syntax is a small, fixed subset, read line by line: anything else is
//! plain text, never a block or an attribute. Raw HTML is text. A link keeps
//! only an `https:`, `http:` or `mailto:` address; any other scheme leaves
//! the words and drops the address. Whatever it makes still passes the
//! block schema (`document::inspect`) like every update.
//!
//! **Out** ([`render`]): a body is read as Markdown for Scout. A mention or
//! an embed is written as `[type:id]` and **never resolved**: what a
//! mentioned record says is not in the text.
//!
//! Nothing here reads what the text means to decide anything: it is syntax
//! only.

mod parse;
mod render;

#[cfg(test)]
use parse::MAX_DEPTH;
pub use parse::{parse, parse_counted};
pub use render::{Rendered, render};

/// The most Markdown `pages.append` and `pages.create` take, in bytes.
pub const MAX_MARKDOWN_BYTES: usize = 16 * 1024;

/// The most Markdown `pages.read_text` answers, in bytes.
pub const MAX_TEXT_BYTES: usize = 64 * 1024;

#[cfg(test)]
mod tests;
