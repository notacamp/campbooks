import { describe, expect, it } from "vitest";

import {
  ASSERTION,
  ASSERTION_JSON,
  ATTESTATION,
  ATTESTATION_JSON,
  CREATION_OPTIONS,
  REQUEST_OPTIONS,
  domError,
  stubWebAuthn,
} from "../../../test/webauthn";

import {
  authenticationJSON,
  autofillSupported,
  ceremonyFailure,
  createPasskey,
  creationOptions,
  deviceName,
  fromBase64Url,
  getPasskey,
  passkeysSupported,
  registrationJSON,
  requestOptions,
  toBase64Url,
  type CreationOptions,
  type RequestOptions,
} from "./webauthn";

describe("webauthn", () => {
  it("round-trips base64url without padding", () => {
    const bytes = new Uint8Array([0, 251, 255, 62, 63, 1]);
    const text = toBase64Url(bytes);
    expect(text).toBe("APv_Pj8B");
    expect([...fromBase64Url(text)]).toEqual([...bytes]);
    expect([...fromBase64Url("AQID=")]).toEqual([1, 2, 3]);
    expect(toBase64Url(new Uint8Array([1, 2, 3]).buffer)).toBe("AQID");
  });

  it("turns the server's options into what navigator.credentials takes", () => {
    const create = creationOptions(CREATION_OPTIONS);
    expect(create.rp).toEqual({ id: "app.example.com", name: "Campbooks" });
    expect(create.user.displayName).toBe("Ana Duarte");
    expect(create.user.id).toBeInstanceOf(Uint8Array);
    expect(create.pubKeyCredParams.map((param) => param.alg)).toEqual([-8, -7, -257]);
    expect(create.excludeCredentials?.[0]?.transports).toEqual(["internal"]);
    expect(create.authenticatorSelection?.userVerification).toBe("required");
    expect(create.attestation).toBe("none");

    const get = requestOptions(REQUEST_OPTIONS);
    expect(get.rpId).toBe("app.example.com");
    expect(get.allowCredentials).toEqual([]);
    expect([...new Uint8Array(get.challenge as ArrayBuffer)]).toEqual([
      ...new TextEncoder().encode("challenge"),
    ]);
  });

  it("sends credentials back as WebAuthn's JSON", () => {
    expect(registrationJSON(ATTESTATION as unknown as PublicKeyCredential)).toEqual(
      ATTESTATION_JSON,
    );
    expect(authenticationJSON(ASSERTION as unknown as PublicKeyCredential)).toEqual(ASSERTION_JSON);
    const anonymous = {
      ...ASSERTION,
      response: { ...ASSERTION.response, userHandle: null },
    };
    expect(
      authenticationJSON(anonymous as unknown as PublicKeyCredential).response.userHandle,
    ).toBeNull();
  });

  it("says what a browser rejection means for the person", () => {
    expect(ceremonyFailure(domError("NotAllowedError"))).toBe("cancelled");
    expect(ceremonyFailure(domError("AbortError"))).toBe("cancelled");
    expect(ceremonyFailure(domError("InvalidStateError"))).toBe("exists");
    expect(ceremonyFailure(domError("NotSupportedError"))).toBe("unsupported");
    expect(ceremonyFailure(domError("SecurityError"))).toBe("unsupported");
    expect(ceremonyFailure(new Error("boom"))).toBe("failed");
    expect(ceremonyFailure("odd")).toBe("failed");
  });

  it("knows when the browser has passkeys and autofill", async () => {
    expect(passkeysSupported()).toBe(false);
    expect(await autofillSupported()).toBe(false);
    stubWebAuthn({ autofill: true });
    expect(passkeysSupported()).toBe(true);
    expect(await autofillSupported()).toBe(true);
  });

  it("asks the browser for a passkey, in a dialog or from autofill", async () => {
    const browser = stubWebAuthn();
    const controller = new AbortController();
    expect(await getPasskey(REQUEST_OPTIONS as RequestOptions)).toEqual(ASSERTION_JSON);
    await getPasskey(REQUEST_OPTIONS, {
      autofill: true,
      signal: controller.signal,
    });
    expect(browser.get.mock.calls[0]?.[0]).not.toHaveProperty("mediation");
    expect(browser.get.mock.calls[1]?.[0]).toMatchObject({
      mediation: "conditional",
      signal: controller.signal,
    });
    expect(await createPasskey(CREATION_OPTIONS as CreationOptions)).toEqual(ATTESTATION_JSON);

    browser.get.mockResolvedValueOnce(null);
    await expect(getPasskey(REQUEST_OPTIONS as RequestOptions)).rejects.toThrow(
      "No passkey was used",
    );
    browser.create.mockResolvedValueOnce(null);
    await expect(createPasskey(CREATION_OPTIONS as CreationOptions)).rejects.toThrow(
      "No passkey was made",
    );
  });

  it("names a passkey after the device it was made on", () => {
    expect(deviceName("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_6)")).toBe("Mac");
    expect(deviceName("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)")).toBe("iPhone");
    expect(deviceName("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")).toBe("Windows");
    expect(deviceName("Mozilla/5.0 (Linux; Android 15)")).toBe("Android");
    expect(deviceName("curl/8")).toBeUndefined();
  });
});
