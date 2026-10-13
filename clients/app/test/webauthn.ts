import { onTestFinished, vi } from "vitest";

// A pretend browser with passkeys for screen tests: `PublicKeyCredential`
// and `navigator.credentials`, answering with fixed demo credentials. The
// real ceremony (signatures, the server's checks) is covered by the Rust
// relying-party tests with a software authenticator, and end to end by the
// recording with Chrome's virtual authenticator.

const bytes = (...values: number[]) => new Uint8Array(values).buffer;

/** A passkey's signature, as `navigator.credentials.get` resolves. */
export const ASSERTION = {
  id: "AQID",
  rawId: bytes(1, 2, 3),
  type: "public-key",
  response: {
    clientDataJSON: bytes(4),
    authenticatorData: bytes(5),
    signature: bytes(6),
    userHandle: bytes(7),
  },
};

/** A new passkey, as `navigator.credentials.create` resolves. */
export const ATTESTATION = {
  id: "AQID",
  rawId: bytes(1, 2, 3),
  type: "public-key",
  response: {
    clientDataJSON: bytes(4),
    attestationObject: bytes(8),
    getTransports: () => ["internal"],
  },
};

/** What `ASSERTION` is sent to the server as. */
export const ASSERTION_JSON = {
  id: "AQID",
  rawId: "AQID",
  type: "public-key",
  response: { clientDataJSON: "BA", authenticatorData: "BQ", signature: "Bg", userHandle: "Bw" },
};

/** What `ATTESTATION` is sent to the server as. */
export const ATTESTATION_JSON = {
  id: "AQID",
  rawId: "AQID",
  type: "public-key",
  response: { clientDataJSON: "BA", attestationObject: "CA", transports: ["internal"] },
};

/** Options as the server sends them (base64url challenge and ids). */
export const REQUEST_OPTIONS = {
  challenge: "Y2hhbGxlbmdl",
  timeout: 300_000,
  rpId: "app.example.com",
  allowCredentials: [],
  userVerification: "required",
};

export const CREATION_OPTIONS = {
  rp: { id: "app.example.com", name: "Campbooks" },
  user: { id: "AZLxoAAAcACAAAAAAAAAoQ", name: "ana@example.com", displayName: "Ana Duarte" },
  challenge: "Y2hhbGxlbmdl",
  pubKeyCredParams: [
    { type: "public-key", alg: -8 },
    { type: "public-key", alg: -7 },
    { type: "public-key", alg: -257 },
  ],
  timeout: 300_000,
  excludeCredentials: [{ type: "public-key", id: "AQID", transports: ["internal"] }],
  authenticatorSelection: {
    residentKey: "required",
    requireResidentKey: true,
    userVerification: "required",
  },
  attestation: "none",
};

export interface FakeWebAuthn {
  get: ReturnType<typeof vi.fn>;
  create: ReturnType<typeof vi.fn>;
}

/**
 * Give the page a browser with passkeys: `autofill` says whether it offers
 * them in a field's suggestions. `get` and `create` resolve the demo
 * credentials unless a test makes them do something else. Undone when the
 * test finishes.
 */
export function stubWebAuthn({ autofill = false }: { autofill?: boolean } = {}): FakeWebAuthn {
  const PublicKeyCredential = Object.assign(vi.fn(), {
    isConditionalMediationAvailable: () => Promise.resolve(autofill),
  });
  vi.stubGlobal("PublicKeyCredential", PublicKeyCredential);
  const fake = {
    get: vi.fn(() => Promise.resolve(ASSERTION)),
    create: vi.fn(() => Promise.resolve(ATTESTATION)),
  };
  Object.defineProperty(globalThis.navigator, "credentials", {
    value: fake,
    configurable: true,
  });
  onTestFinished(() => {
    Reflect.deleteProperty(globalThis.navigator, "credentials");
  });
  return fake;
}

/** A browser error as `navigator.credentials` rejects with. */
export function domError(name: string): DOMException {
  return new DOMException(name, name);
}
