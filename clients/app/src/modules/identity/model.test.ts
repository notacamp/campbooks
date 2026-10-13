import { afterEach, describe, expect, it } from "vitest";

import { writeTabValue } from "~/lib/storage/tab";

import {
  isInvitation,
  rememberedReturnTo,
  rememberReturnTo,
  safeReturnTo,
  signInError,
  startAgainUrl,
  startUrl,
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
