//! The module's part of the access catalogue (ADR 0017).
//!
//! The account resources (ways in, passkeys, consents held for a workspace,
//! sessions) belong to the account they were made by; every API operation
//! on them checks `authz::require_account`, and each feature reads only the
//! caller's own rows. Per-instance facts through `authz::authorize` arrive
//! with the identity migration (campbooks-cloud#117, PR B). The protocol
//! endpoints are public by nature: they authenticate what they act on
//! themselves (a client secret, a PKCE verifier, a signed-in session).

use campbooks_kernel::{
    authz::{
        Action::{Create, Delete, Read, Write},
        Grant, Grantee, Owner, ResourceType, Resources, Tenancy,
    },
    tenancy::Role,
};

/// The module's resource types.
static TYPES: [ResourceType; 11] = [
    ResourceType {
        owner: Owner::Account,
        owner_actions: &[Read, Write, Delete],
        ..ResourceType::new(
            "identity.provider_identity",
            "A way in: a provider account (a mailbox) that signs its owner in. Only its \
             account sees, links or removes it.",
            Tenancy::Account,
        )
    },
    ResourceType {
        owner: Owner::Account,
        owner_actions: &[Read, Write, Delete],
        ..ResourceType::new(
            "identity.passkey",
            "A passkey of an account: only its account lists, adds, renames or removes it.",
            Tenancy::Account,
        )
    },
    ResourceType {
        owner: Owner::Account,
        owner_actions: &[Read, Write, Delete],
        ..ResourceType::new(
            "identity.held_grant",
            "A mailbox consent given at sign-up and held until its account puts it in a \
             workspace: only that account sees, adopts or discards it.",
            Tenancy::Account,
        )
    },
    ResourceType {
        owner: Owner::Account,
        owner_actions: &[Read, Delete],
        grants: &[Grant::new(Grantee::Anyone, &[Delete])],
        ..ResourceType::new(
            "identity.session",
            "A signed-in session of an account at the issuer. Ending one (RP-initiated \
             logout) proves the session with its ID token hint.",
            Tenancy::Account,
        )
    },
    ResourceType {
        grants: &[Grant::new(
            Grantee::WorkspaceRole(Role::Admin),
            &[Create, Read, Write, Delete],
        )],
        ..ResourceType::new(
            "identity.client",
            "An OAuth client a workspace registered (an integration or agent bound to it): \
             its owners and admins create, list, rotate and remove them.",
            Tenancy::Workspace,
        )
    },
    ResourceType {
        owner: Owner::Account,
        owner_actions: &[Read, Delete],
        ..ResourceType::new(
            "identity.consent",
            "An app an account allowed to act for it (a connected app): only that account \
             lists or revokes it.",
            Tenancy::Account,
        )
    },
    ResourceType {
        grants: &[Grant::new(Grantee::Anyone, &[Create])],
        ..ResourceType::new(
            "identity.registration",
            "Dynamic client registration (RFC 7591) for agents: public, rate-limited, and the \
             registered client holds no access until a person consents.",
            Tenancy::Public,
        )
    },
    ResourceType {
        grants: &[Grant::new(Grantee::Anyone, &[Read])],
        ..ResourceType::new(
            "identity.signing_key",
            "The issuer's public signing keys (JWKS): public. Private keys never leave the \
             module.",
            Tenancy::Public,
        )
    },
    ResourceType {
        grants: &[Grant::new(Grantee::Anyone, &[Read])],
        ..ResourceType::new(
            "identity.discovery",
            "The issuer's discovery documents: public.",
            Tenancy::Public,
        )
    },
    ResourceType {
        grants: &[Grant::new(Grantee::Anyone, &[Read, Write])],
        ..ResourceType::new(
            "identity.sign_in",
            "A sign-in or sign-up in progress (authorization requests, provider round trips, \
             passkey ceremonies): anyone may start one; each step proves the one before.",
            Tenancy::Public,
        )
    },
    ResourceType {
        grants: &[Grant::new(Grantee::Anyone, &[Write, Delete])],
        ..ResourceType::new(
            "identity.token",
            "Tokens issued and revoked at the token endpoint, which authenticates the client \
             and the grant (code + PKCE, refresh token) itself.",
            Tenancy::Public,
        )
    },
];

/// The module's resources, for its descriptor.
pub fn resources() -> Resources {
    Resources::declared(&TYPES)
}
