//! Types and helpers shared by several features of this module.

pub mod index;
pub mod language;
pub mod query;
pub mod segments;
pub mod text;

campbooks_kernel::define_id!(
    /// A row of `search.groups`.
    GroupId
);
campbooks_kernel::define_id!(
    /// A row of `search.entries`.
    EntryId
);
