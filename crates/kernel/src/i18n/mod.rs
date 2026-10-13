//! Internationalization: the locales Campbooks ships, choosing one per
//! request, and the message catalogues server text is rendered from.
//!
//! - [`Locale`] is `en`, `pt-PT`, `es-ES` or `fr`, always the full tag.
//! - Per request it is resolved from the user's saved locale (the
//!   [`LocalePreference`] port, implemented by the `workspaces` module), then
//!   `Accept-Language`, then `en`. Handlers that render text take it as an
//!   extractor (`locale: Locale`); it is resolved only when asked for.
//! - Text comes from a [`Catalogue`] per crate, declared with
//!   [`catalogue!`](crate::catalogue) over its `i18n/<locale>.json` files
//!   (the client's i18next JSON format) and rendered with [`Catalogue::t`].
//! - Errors carry a [`Reason`] (a stable code + a message); the [`localize`]
//!   middleware renders every problem+json response in the request's locale.
//! - Jobs have no request: a job that renders text carries the recipient's
//!   `Locale` in its payload and calls `MESSAGES.t(job.locale, …)`.
//!
//! `cargo xtask i18n` holds every catalogue to [`check_catalogue`].

mod catalogue;
mod check;
mod locale;
mod request;

pub use catalogue::{Arg, COUNT, Catalogue, Reason, plural_category};
pub use check::check_catalogue;
pub use locale::{Locale, UnknownLocale};
pub use request::{
    LocaleFuture, LocalePreference, LocalePreferencePort, NoLocalePreference, localize,
};

/// The kernel's own messages: HTTP status titles and the reasons of the
/// errors the kernel raises (`crates/kernel/i18n/`).
pub static MESSAGES: Catalogue = crate::catalogue!("kernel");

/// Declares a crate's [`Catalogue`] from its `i18n/en.json`,
/// `i18n/pt-PT.json`, `i18n/es-ES.json` and `i18n/fr.json` (next to its
/// `Cargo.toml`), embedded at compile time:
///
/// ```ignore
/// pub static MESSAGES: campbooks_kernel::i18n::Catalogue = campbooks_kernel::catalogue!("mail");
/// ```
#[macro_export]
macro_rules! catalogue {
    ($namespace:expr) => {
        $crate::i18n::Catalogue::new(
            $namespace,
            [
                ::core::include_str!(::core::concat!(
                    ::core::env!("CARGO_MANIFEST_DIR"),
                    "/i18n/en.json"
                )),
                ::core::include_str!(::core::concat!(
                    ::core::env!("CARGO_MANIFEST_DIR"),
                    "/i18n/pt-PT.json"
                )),
                ::core::include_str!(::core::concat!(
                    ::core::env!("CARGO_MANIFEST_DIR"),
                    "/i18n/es-ES.json"
                )),
                ::core::include_str!(::core::concat!(
                    ::core::env!("CARGO_MANIFEST_DIR"),
                    "/i18n/fr.json"
                )),
            ],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_catalogue_is_at_parity() {
        assert_eq!(MESSAGES.check(), Vec::<String>::new());
    }
}
