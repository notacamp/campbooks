//! Scout's handbook: how Campbooks works, as the model reads it before it
//! answers a member's question (campbooks-cloud#305).
//!
//! Each module Scout explains owns its section, next to its code
//! (`crates/modules/<module>/handbook.md`, exported as `HANDBOOK`). The
//! server composes them, in a fixed order, after this module's own core
//! section and hands them over with the module's dependencies: this module
//! imports no other. The handbook's version is derived from every part, so
//! a changed section is a new version on its own.

/// This module's own section: who Scout is, what it may and may not do, its
/// tools, and that what it reads is data, never instructions.
pub const CORE: &str = include_str!("../handbook.md");

/// The most bytes one section may hold (checked by the server's tests, for
/// every module's section).
pub const MAX_SECTION_BYTES: usize = 8 * 1024;

/// One module's section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    /// The module that owns it.
    pub module: &'static str,
    /// Its text (Markdown, English).
    pub text: &'static str,
}

/// The handbook: the core section, then the modules' sections in the order
/// the server gave them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handbook {
    sections: Vec<Section>,
}

impl Default for Handbook {
    /// The core section alone.
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Handbook {
    /// The core section followed by `sections`, in this order.
    pub fn new(sections: Vec<Section>) -> Self {
        let mut all = vec![Section {
            module: crate::NAME,
            text: CORE,
        }];
        all.extend(sections);
        Self { sections: all }
    }

    /// Every section, the core first.
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// The whole text the model reads.
    pub fn text(&self) -> String {
        self.sections
            .iter()
            .map(|section| section.text.trim())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Its version: a hash of every part, in order (FNV-1a, 64 bits),
    /// stable across builds and Rust versions. Logged with each question.
    pub fn version(&self) -> String {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0100_0000_01b3;
        let mut hash = OFFSET;
        for section in &self.sections {
            for byte in section
                .module
                .bytes()
                .chain([0])
                .chain(section.text.bytes())
                .chain([0])
            {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(PRIME);
            }
        }
        format!("{hash:016x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_core_comes_first_and_fits() {
        let handbook = Handbook::new(vec![Section {
            module: "conversations",
            text: "## Conversations\n\nStreams.",
        }]);
        assert_eq!(handbook.sections()[0].module, "assistant");
        assert!(CORE.len() <= MAX_SECTION_BYTES, "{}", CORE.len());
        assert!(handbook.text().ends_with("Streams."));
    }

    #[test]
    fn any_change_to_a_part_is_a_new_version() {
        let one = Handbook::new(vec![Section {
            module: "attention",
            text: "Needs.",
        }]);
        let other = Handbook::new(vec![Section {
            module: "attention",
            text: "Needs!",
        }]);
        let moved = Handbook::new(vec![Section {
            module: "conversations",
            text: "Needs.",
        }]);
        assert_ne!(one.version(), other.version());
        assert_ne!(one.version(), moved.version());
        assert_eq!(one.version(), one.clone().version());
        assert_eq!(one.version().len(), 16);
    }
}
