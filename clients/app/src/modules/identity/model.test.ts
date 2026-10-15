import { afterEach, describe, expect, it } from "vitest";

import { writeTabValue } from "~/lib/storage/tab";

import {
  addedRecently,
  isInvitation,
  isReturnAddress,
  linkResult,
  rememberedReturnTo,
  rememberReturnTo,
  safeReturnTo,
  signInError,
  startAgainUrl,
  startUrl,
  stepUpUrl,
  waysIn,
} from "./model";

describe("identity model", () => {
  afterEach(() => {
    writeTabValue("signIn.returnTo", "");
  });

  it("shows the known ways in, in the server's order, at most three providers", () => {
    expect(waysIn(["zoho", "passkey", "google", "facebook", "microsoft"])).toEqual({
      providers: ["zoho", "google", "microsoft"],
      passkey: true,
    });
    expect(waysIn([])).toEqual({ providers: [], passkey: false });
  });

  it("starts a provider round trip for the waiting request", () => {
    expect(startUrl("google", "r 1")).toBe("/auth/start/google?request=r%201");
  });

  it("only comes back to a same-origin page that isn't a sign-in screen", () => {
    expect(safeReturnTo("/w/1?tab=a#b")).toBe("/w/1?tab=a#b");
    for (const unsafe of [
      undefined,
      null,
      "",
      "https://evil.example.com",
      "//evil.example.com",
      String.raw`/\evil.example.com`,
      "/sign-in",
      "/sign-in?request=x",
      "/sign-in/new",
    ]) {
      expect(safeReturnTo(unsafe)).toBe("/");
    }
  });

  it("remembers where to come back to for this tab, and starts again from there", () => {
    expect(rememberedReturnTo()).toBe("/");
    rememberReturnTo("/invitations/accept?token=t");

    expect(rememberedReturnTo()).toBe("/invitations/accept?token=t");
    expect(isInvitation(rememberedReturnTo())).toBe(true);
    expect(startAgainUrl()).toBe("/bff/login?return_to=%2Finvitations%2Faccept%3Ftoken%3Dt");
  });

  it("explains known callback errors and calls the rest unknown", () => {
    expect(signInError("invalid_state")).toBe("invalid_state");
    expect(signInError("something_new")).toBe("unknown");
  });
});

describe("linkResult", () => {
  it("reads what the front door came back with", () => {
    expect(linkResult({ linked: "zoho" })).toEqual({ kind: "linked", provider: "zoho" });
    expect(linkResult({ linked: "yahoo" })).toBeUndefined();
    expect(linkResult({ connected: "c1" })).toEqual({ kind: "connected" });
    expect(linkResult({ link_error: "last_way_in_ever", linked: "zoho" })).toEqual({
      kind: "error",
      error: "unknown",
    });
    expect(linkResult({ link_error: "sign_in_again" })).toEqual({
      kind: "error",
      error: "sign_in_again",
    });
    expect(linkResult({})).toBeUndefined();
  });

  it("signs in again for at most ten minutes, coming back here", () => {
    expect(stepUpUrl("/w/1/settings/sign-in")).toBe(
      "/bff/login?return_to=%2Fw%2F1%2Fsettings%2Fsign-in&max_age=600",
    );
    expect(stepUpUrl("https://evil.example.com")).toBe("/bff/login?return_to=%2F&max_age=600");
  });

  it("signs in anew for a passkey (max_age=0)", () => {
    expect(stepUpUrl("/w/1/settings/sign-in", { fresh: true })).toBe(
      "/bff/login?return_to=%2Fw%2F1%2Fsettings%2Fsign-in&max_age=0",
    );
  });
});

describe("addedRecently", () => {
  const now = new Date("2026-10-14T12:00:00Z");

  it("is true for the last seven days only", () => {
    expect(addedRecently("2026-10-14T11:00:00Z", now)).toBe(true);
    expect(addedRecently("2026-10-08T12:00:01Z", now)).toBe(true);
    expect(addedRecently("2026-10-07T12:00:00Z", now)).toBe(false);
    expect(addedRecently("2026-10-15T12:00:00Z", now)).toBe(false);
  });
});

describe("isReturnAddress", () => {
  it("takes https and loopback http, nothing else", () => {
    for (const ok of [
      "https://dashboard.example.com/cb",
      "http://127.0.0.1:5555/callback",
      "http://[::1]:8000/cb",
      "http://localhost:3000/cb",
    ]) {
      expect(isReturnAddress(ok)).toBe(true);
    }
    for (const bad of [
      "http://dashboard.example.com/cb",
      "https://dashboard.example.com/cb#x",
      "https://user:pw@dashboard.example.com/cb",
      "com.example.app:/cb",
      "not a url",
    ]) {
      expect(isReturnAddress(bad)).toBe(false);
    }
  });
});
