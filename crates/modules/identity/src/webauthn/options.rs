//! The options a browser is given for a ceremony, in WebAuthn L3's JSON
//! form (`PublicKeyCredentialCreationOptionsJSON` and
//! `PublicKeyCredentialRequestOptionsJSON`, §5.1.8 / §5.1.9): binary values
//! are base64url without padding, names are camelCase, so the web client
//! can pass them to `navigator.credentials` as they are.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{ALGORITHMS, CEREMONY_TIMEOUT, RelyingParty};

/// The relying party, as the browser shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RelyingPartyEntity {
    /// The relying-party id (the public URL's host).
    pub id: String,
    /// Shown by the browser: `Campbooks`.
    pub name: String,
}

/// The account the credential is for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct UserEntity {
    /// The user handle: the account id's 16 bytes, base64url. It holds no
    /// personal information.
    pub id: String,
    /// What the passkey is saved under (an address or a name).
    pub name: String,
    /// The person's name.
    #[serde(rename = "displayName")]
    pub display_name: String,
}

/// An algorithm the relying party accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CredentialParameter {
    /// `public-key`.
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// The COSE algorithm.
    pub alg: i64,
}

/// A credential, by id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CredentialDescriptor {
    /// `public-key`.
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// The credential id, base64url.
    pub id: String,
    /// Transports the authenticator reported, when it did.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub transports: Vec<String>,
}

/// What the authenticator must do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AuthenticatorSelection {
    /// `required`: a discoverable credential (a passkey).
    #[serde(rename = "residentKey")]
    pub resident_key: &'static str,
    /// `true`, for browsers that predate `residentKey`.
    #[serde(rename = "requireResidentKey")]
    pub require_resident_key: bool,
    /// `required`.
    #[serde(rename = "userVerification")]
    pub user_verification: &'static str,
}

/// `PublicKeyCredentialCreationOptionsJSON`: create a passkey.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct CreationOptions {
    /// The relying party.
    pub rp: RelyingPartyEntity,
    /// The account.
    pub user: UserEntity,
    /// The challenge, base64url (single use, five minutes).
    pub challenge: String,
    /// EdDSA, ES256, RS256.
    #[serde(rename = "pubKeyCredParams")]
    pub pub_key_cred_params: Vec<CredentialParameter>,
    /// Milliseconds.
    pub timeout: u64,
    /// The account's passkeys, so an authenticator holding one says so
    /// instead of making a second.
    #[serde(rename = "excludeCredentials")]
    pub exclude_credentials: Vec<CredentialDescriptor>,
    /// Discoverable, with user verification.
    #[serde(rename = "authenticatorSelection")]
    pub authenticator_selection: AuthenticatorSelection,
    /// `none`.
    pub attestation: &'static str,
}

/// `PublicKeyCredentialRequestOptionsJSON`: sign in with a passkey.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct RequestOptions {
    /// The challenge, base64url (single use, five minutes).
    pub challenge: String,
    /// Milliseconds.
    pub timeout: u64,
    /// The relying-party id.
    #[serde(rename = "rpId")]
    pub rp_id: String,
    /// Empty: any of this site's passkeys (discoverable credentials).
    #[serde(rename = "allowCredentials")]
    pub allow_credentials: Vec<CredentialDescriptor>,
    /// `required`.
    #[serde(rename = "userVerification")]
    pub user_verification: &'static str,
}

/// The name browsers show for the relying party.
const RP_NAME: &str = "Campbooks";

fn timeout_ms() -> u64 {
    u64::try_from(CEREMONY_TIMEOUT.num_milliseconds()).unwrap_or(300_000)
}

impl RelyingParty {
    /// Options to create a passkey for account `user` (its id is the user
    /// handle), saved under `name`, excluding the credentials in `exclude`.
    pub fn creation_options(
        &self,
        challenge: &str,
        user: Uuid,
        name: &str,
        display_name: &str,
        exclude: Vec<CredentialDescriptor>,
    ) -> CreationOptions {
        CreationOptions {
            rp: RelyingPartyEntity {
                id: self.id.clone(),
                name: RP_NAME.to_owned(),
            },
            user: UserEntity {
                id: URL_SAFE_NO_PAD.encode(user.as_bytes()),
                name: name.to_owned(),
                display_name: display_name.to_owned(),
            },
            challenge: challenge.to_owned(),
            pub_key_cred_params: ALGORITHMS
                .iter()
                .map(|alg| CredentialParameter {
                    kind: "public-key",
                    alg: *alg,
                })
                .collect(),
            timeout: timeout_ms(),
            exclude_credentials: exclude,
            authenticator_selection: AuthenticatorSelection {
                resident_key: "required",
                require_resident_key: true,
                user_verification: "required",
            },
            attestation: "none",
        }
    }

    /// Options to sign in with any of this site's passkeys.
    pub fn request_options(&self, challenge: &str) -> RequestOptions {
        RequestOptions {
            challenge: challenge.to_owned(),
            timeout: timeout_ms(),
            rp_id: self.id.clone(),
            allow_credentials: Vec::new(),
            user_verification: "required",
        }
    }
}

/// A stored credential as `excludeCredentials` names it.
pub fn descriptor(credential_id: &[u8], transports: Vec<String>) -> CredentialDescriptor {
    CredentialDescriptor {
        kind: "public-key",
        id: URL_SAFE_NO_PAD.encode(credential_id),
        transports,
    }
}
