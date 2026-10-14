import type { Schema } from "~/lib/api";

// The browser side of passkeys: the server speaks WebAuthn's JSON forms
// (base64url for every binary value), `navigator.credentials` speaks
// ArrayBuffers. These convert between the two by hand, so they work in every
// browser with passkeys, not only those with `parseCreationOptionsFromJSON`.

export type CreationOptions = Schema<"CreationOptions">;
export type RequestOptions = Schema<"RequestOptions">;
export type RegistrationJSON = Schema<"RegistrationCredential">;
export type AuthenticationJSON = Schema<"AuthenticationCredential">;

/** Bytes as base64url, without padding. */
export function toBase64Url(bytes: ArrayBuffer | Uint8Array): string {
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  let binary = "";
  for (const byte of view) {
    binary += String.fromCodePoint(byte);
  }
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/u, "");
}

/** base64url (with or without padding) as bytes. */
export function fromBase64Url(text: string): Uint8Array<ArrayBuffer> {
  const base64 = text.replace(/=+$/u, "").replaceAll("-", "+").replaceAll("_", "/");
  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(new ArrayBuffer(binary.length));
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.codePointAt(index) ?? 0;
  }
  return bytes;
}

/** Whether this browser can use passkeys at all. */
export function passkeysSupported(): boolean {
  return (
    typeof globalThis.PublicKeyCredential === "function" &&
    "credentials" in globalThis.navigator &&
    typeof globalThis.navigator.credentials.get === "function"
  );
}

/** Whether this browser offers passkeys in a field's autofill (conditional UI). */
export async function autofillSupported(): Promise<boolean> {
  if (!passkeysSupported()) {
    return false;
  }
  const check = (
    globalThis.PublicKeyCredential as unknown as {
      isConditionalMediationAvailable?: () => Promise<boolean>;
    }
  ).isConditionalMediationAvailable;
  return typeof check === "function" ? check.call(globalThis.PublicKeyCredential) : false;
}

const descriptor = (credential: { type: string; id: string; transports?: string[] }) => ({
  type: credential.type as PublicKeyCredentialType,
  id: fromBase64Url(credential.id),
  ...(credential.transports === undefined
    ? {}
    : { transports: credential.transports as AuthenticatorTransport[] }),
});

/** The server's creation options, as `navigator.credentials.create` takes them. */
export function creationOptions(json: CreationOptions): PublicKeyCredentialCreationOptions {
  return {
    rp: json.rp,
    user: {
      id: fromBase64Url(json.user.id),
      name: json.user.name,
      displayName: json.user.displayName,
    },
    challenge: fromBase64Url(json.challenge),
    pubKeyCredParams: json.pubKeyCredParams.map((param) => ({
      type: param.type as PublicKeyCredentialType,
      alg: param.alg,
    })),
    timeout: json.timeout,
    excludeCredentials: json.excludeCredentials.map(descriptor),
    authenticatorSelection: {
      residentKey: json.authenticatorSelection.residentKey as ResidentKeyRequirement,
      requireResidentKey: json.authenticatorSelection.requireResidentKey,
      userVerification: json.authenticatorSelection.userVerification as UserVerificationRequirement,
    },
    attestation: json.attestation as AttestationConveyancePreference,
  };
}

/** The server's request options, as `navigator.credentials.get` takes them. */
export function requestOptions(json: RequestOptions): PublicKeyCredentialRequestOptions {
  return {
    challenge: fromBase64Url(json.challenge),
    timeout: json.timeout,
    rpId: json.rpId,
    allowCredentials: json.allowCredentials.map(descriptor),
    userVerification: json.userVerification as UserVerificationRequirement,
  };
}

/** A new passkey, as the server reads it (`RegistrationResponseJSON`). */
export function registrationJSON(credential: PublicKeyCredential): RegistrationJSON {
  const response = credential.response as AuthenticatorAttestationResponse;
  const transports =
    typeof response.getTransports === "function" ? response.getTransports() : undefined;
  return {
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      clientDataJSON: toBase64Url(response.clientDataJSON),
      attestationObject: toBase64Url(response.attestationObject),
      ...(transports === undefined ? {} : { transports }),
    },
  };
}

/** A passkey's signature, as the server reads it (`AuthenticationResponseJSON`). */
export function authenticationJSON(credential: PublicKeyCredential): AuthenticationJSON {
  const response = credential.response as AuthenticatorAssertionResponse;
  return {
    id: credential.id,
    rawId: toBase64Url(credential.rawId),
    type: credential.type,
    response: {
      clientDataJSON: toBase64Url(response.clientDataJSON),
      authenticatorData: toBase64Url(response.authenticatorData),
      signature: toBase64Url(response.signature),
      userHandle: response.userHandle === null ? null : toBase64Url(response.userHandle),
    },
  };
}

/** How a passkey ceremony in the browser ended without a credential. */
export type CeremonyFailure = "cancelled" | "unsupported" | "exists" | "failed";

/**
 * What a `navigator.credentials` rejection means for the person: cancelled
 * (or timed out, which browsers report the same way), an authenticator that
 * already holds a passkey for this account, no passkey support, or anything
 * else.
 */
export function ceremonyFailure(error: unknown): CeremonyFailure {
  const name = error instanceof DOMException || error instanceof Error ? error.name : "";
  switch (name) {
    case "NotAllowedError":
    case "AbortError":
      return "cancelled";
    case "InvalidStateError":
      return "exists";
    case "NotSupportedError":
    case "SecurityError":
      return "unsupported";
    default:
      return "failed";
  }
}

/** A credential the browser returned that is a passkey's (not a password). */
function isPasskeyCredential(credential: Credential | null): credential is PublicKeyCredential {
  return credential !== null && "rawId" in credential && "response" in credential;
}

/** Ask the browser for a new passkey for `options`. */
export async function createPasskey(json: CreationOptions): Promise<RegistrationJSON> {
  const credential = await globalThis.navigator.credentials.create({
    publicKey: creationOptions(json),
  });
  if (!isPasskeyCredential(credential)) {
    throw new DOMException("No passkey was made", "NotAllowedError");
  }
  return registrationJSON(credential);
}

/**
 * Ask the browser to sign `options` with one of this site's passkeys: in a
 * dialog, or (`autofill`) from a field's suggestions, until `signal` aborts.
 */
export async function getPasskey(
  json: RequestOptions,
  { autofill = false, signal }: { autofill?: boolean; signal?: AbortSignal } = {},
): Promise<AuthenticationJSON> {
  const credential = await globalThis.navigator.credentials.get({
    publicKey: requestOptions(json),
    ...(autofill ? { mediation: "conditional" as CredentialMediationRequirement } : {}),
    ...(signal === undefined ? {} : { signal }),
  });
  if (!isPasskeyCredential(credential)) {
    throw new DOMException("No passkey was used", "NotAllowedError");
  }
  return authenticationJSON(credential);
}

/**
 * A name for a passkey made on this device ("Mac", "iPhone", …), from the
 * user agent; `undefined` when it can't tell.
 */
export function deviceName(userAgent: string = globalThis.navigator.userAgent): string | undefined {
  const known: [RegExp, string][] = [
    [/iPhone/u, "iPhone"],
    [/iPad/u, "iPad"],
    [/Android/u, "Android"],
    [/CrOS/u, "Chromebook"],
    [/Macintosh|Mac OS X/u, "Mac"],
    [/Windows/u, "Windows"],
    [/Linux/u, "Linux"],
  ];
  return known.find(([pattern]) => pattern.test(userAgent))?.[1];
}
