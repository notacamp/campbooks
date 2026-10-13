// A pretend browser with passkeys for screen stories (demo data, no
// authenticator): `navigator.credentials` answers with fixed credentials, or
// fails as a story says, and autofill waits until the page aborts it.

/** How the pretend browser answers a dialog ceremony. */
export type MockCeremony = "ok" | "cancel" | "exists";

export interface MockWebAuthn {
  /** Whether a field's autofill offers passkeys (conditional UI). */
  autofill?: boolean;
  /** What the passkey dialog does. */
  ceremony?: MockCeremony;
}

const bytes = (...values: number[]) => new Uint8Array(values).buffer;

const ASSERTION = {
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

const ATTESTATION = {
  id: "AQID",
  rawId: bytes(1, 2, 3),
  type: "public-key",
  response: {
    clientDataJSON: bytes(4),
    attestationObject: bytes(8),
    getTransports: () => ["internal"],
  },
};

/** Request options as the server sends them, for the stories' mocked endpoints. */
export const REQUEST_OPTIONS = {
  challenge: "Y2hhbGxlbmdl",
  timeout: 300_000,
  rpId: "app.example.com",
  allowCredentials: [],
  userVerification: "required",
};

/** Creation options as the server sends them. */
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
  excludeCredentials: [],
  authenticatorSelection: {
    residentKey: "required",
    requireResidentKey: true,
    userVerification: "required",
  },
  attestation: "none",
};

function answer<T>(ceremony: MockCeremony, credential: T): Promise<T> {
  switch (ceremony) {
    case "cancel":
      return Promise.reject(new DOMException("Cancelled", "NotAllowedError"));
    case "exists":
      return Promise.reject(new DOMException("Already there", "InvalidStateError"));
    case "ok":
      return Promise.resolve(credential);
  }
}

/** Install the pretend browser; returns the function that takes it away. */
export function mockWebAuthn({ autofill = true, ceremony = "ok" }: MockWebAuthn): () => void {
  const credentials = {
    get: (options?: CredentialRequestOptions) => {
      if (options?.mediation === "conditional") {
        // Waits for a pick that never comes, until the page aborts it.
        return new Promise((_resolve, reject) => {
          options.signal?.addEventListener("abort", () => {
            reject(new DOMException("Aborted", "AbortError"));
          });
        });
      }
      return answer(ceremony, ASSERTION);
    },
    create: () => answer(ceremony, ATTESTATION),
  };
  const hadCredential = "PublicKeyCredential" in globalThis;
  const original = hadCredential ? globalThis.PublicKeyCredential : undefined;
  Object.defineProperty(globalThis, "PublicKeyCredential", {
    value: Object.assign(
      function PublicKeyCredential() {
        throw new TypeError("Illegal constructor");
      },
      { isConditionalMediationAvailable: () => Promise.resolve(autofill) },
    ),
    configurable: true,
    writable: true,
  });
  Object.defineProperty(globalThis.navigator, "credentials", {
    value: credentials,
    configurable: true,
  });
  return () => {
    Reflect.deleteProperty(globalThis.navigator, "credentials");
    if (original === undefined) {
      Reflect.deleteProperty(globalThis, "PublicKeyCredential");
    } else {
      globalThis.PublicKeyCredential = original;
    }
  };
}
