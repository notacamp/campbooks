//! Strongly typed identifiers.
//!
//! Every entity gets its own ID type so a `MailboxId` can never be passed
//! where a `ThreadId` is expected. IDs are UUIDv7 (time-ordered, index
//! friendly) and are how modules reference each other's records.
//!
//! ```
//! campbooks_kernel::define_id!(
//!     /// A connected mailbox.
//!     MailboxId
//! );
//!
//! let id = MailboxId::new();
//! let parsed: MailboxId = id.to_string().parse().unwrap();
//! assert_eq!(id, parsed);
//! ```

/// Re-exports used by [`define_id!`] so calling crates need no extra deps.
#[doc(hidden)]
pub mod __private {
    pub use serde;
    pub use sqlx;
    pub use utoipa;
    pub use uuid;
}

/// Define a UUIDv7 newtype ID with serde, `Display`/`FromStr`, `sqlx`
/// (Postgres `uuid`, transparent) and `utoipa` schema support.
#[macro_export]
macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($crate::ids::__private::uuid::Uuid);

        impl $name {
            /// A new, time-ordered (v7) ID.
            #[must_use]
            pub fn new() -> Self {
                Self($crate::ids::__private::uuid::Uuid::now_v7())
            }

            /// Wrap an existing UUID (e.g. one read from a URL or a peer module's event).
            #[must_use]
            pub const fn from_uuid(uuid: $crate::ids::__private::uuid::Uuid) -> Self {
                Self(uuid)
            }

            /// The underlying UUID.
            #[must_use]
            pub const fn as_uuid(&self) -> &$crate::ids::__private::uuid::Uuid {
                &self.0
            }
        }

        impl ::core::default::Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(&self.0, f)
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = $crate::ids::__private::uuid::Error;

            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                s.parse().map(Self)
            }
        }

        impl ::core::convert::From<$crate::ids::__private::uuid::Uuid> for $name {
            fn from(uuid: $crate::ids::__private::uuid::Uuid) -> Self {
                Self(uuid)
            }
        }

        impl ::core::convert::From<$name> for $crate::ids::__private::uuid::Uuid {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl $crate::ids::__private::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
            where
                S: $crate::ids::__private::serde::Serializer,
            {
                $crate::ids::__private::serde::Serialize::serialize(&self.0, serializer)
            }
        }

        impl<'de> $crate::ids::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
            where
                D: $crate::ids::__private::serde::Deserializer<'de>,
            {
                <$crate::ids::__private::uuid::Uuid as $crate::ids::__private::serde::Deserialize>::deserialize(deserializer).map(Self)
            }
        }

        impl $crate::ids::__private::sqlx::Type<$crate::ids::__private::sqlx::Postgres> for $name {
            fn type_info() -> $crate::ids::__private::sqlx::postgres::PgTypeInfo {
                <$crate::ids::__private::uuid::Uuid as $crate::ids::__private::sqlx::Type<
                    $crate::ids::__private::sqlx::Postgres,
                >>::type_info()
            }

            fn compatible(ty: &$crate::ids::__private::sqlx::postgres::PgTypeInfo) -> bool {
                <$crate::ids::__private::uuid::Uuid as $crate::ids::__private::sqlx::Type<
                    $crate::ids::__private::sqlx::Postgres,
                >>::compatible(ty)
            }
        }

        impl $crate::ids::__private::sqlx::postgres::PgHasArrayType for $name {
            fn array_type_info() -> $crate::ids::__private::sqlx::postgres::PgTypeInfo {
                <$crate::ids::__private::uuid::Uuid as $crate::ids::__private::sqlx::postgres::PgHasArrayType>::array_type_info()
            }
        }

        impl $crate::ids::__private::sqlx::Encode<'_, $crate::ids::__private::sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut $crate::ids::__private::sqlx::postgres::PgArgumentBuffer,
            ) -> ::core::result::Result<
                $crate::ids::__private::sqlx::encode::IsNull,
                $crate::ids::__private::sqlx::error::BoxDynError,
            > {
                <$crate::ids::__private::uuid::Uuid as $crate::ids::__private::sqlx::Encode<
                    '_,
                    $crate::ids::__private::sqlx::Postgres,
                >>::encode_by_ref(&self.0, buf)
            }
        }

        impl<'r> $crate::ids::__private::sqlx::Decode<'r, $crate::ids::__private::sqlx::Postgres> for $name {
            fn decode(
                value: $crate::ids::__private::sqlx::postgres::PgValueRef<'r>,
            ) -> ::core::result::Result<Self, $crate::ids::__private::sqlx::error::BoxDynError> {
                <$crate::ids::__private::uuid::Uuid as $crate::ids::__private::sqlx::Decode<
                    'r,
                    $crate::ids::__private::sqlx::Postgres,
                >>::decode(value)
                .map(Self)
            }
        }

        impl $crate::ids::__private::utoipa::PartialSchema for $name {
            fn schema() -> $crate::ids::__private::utoipa::openapi::RefOr<
                $crate::ids::__private::utoipa::openapi::schema::Schema,
            > {
                use $crate::ids::__private::utoipa::openapi::schema::{
                    KnownFormat, ObjectBuilder, SchemaFormat, Type,
                };
                ObjectBuilder::new()
                    .schema_type(Type::String)
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
                    .description(Some(concat!(stringify!($name), " (UUIDv7)")))
                    .into()
            }
        }

        impl $crate::ids::__private::utoipa::ToSchema for $name {}
    };
}

#[cfg(test)]
mod tests {
    crate::define_id!(
        /// Test ID.
        WidgetId
    );

    #[test]
    fn new_ids_are_v7_and_ordered() {
        let a = WidgetId::new();
        let b = WidgetId::new();
        assert_eq!(a.as_uuid().get_version_num(), 7);
        assert!(a < b);
    }

    #[test]
    fn display_and_from_str_round_trip() {
        let id = WidgetId::new();
        assert_eq!(id.to_string().parse::<WidgetId>().unwrap(), id);
        assert!("not-a-uuid".parse::<WidgetId>().is_err());
    }

    #[test]
    fn serde_is_a_plain_uuid_string() {
        let id = WidgetId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<WidgetId>(&json).unwrap(), id);
    }

    #[test]
    fn schema_is_named_and_uuid_formatted() {
        use utoipa::{PartialSchema, ToSchema};
        assert_eq!(WidgetId::name(), "WidgetId");
        let schema = serde_json::to_value(WidgetId::schema()).unwrap();
        assert_eq!(schema["format"], "uuid");
    }
}
