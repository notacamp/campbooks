import { describe, expect, it, vi } from "vitest";

import { mockApi } from "../../../test/render";

import {
  createScreenViews,
  screenRoute,
  startScreenViews,
  trackingRefused,
  type PrivacySignals,
  type SendScreenView,
} from "./screen-views";

const WORKSPACE = "0192a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";

function setup(signals: PrivacySignals = {}, outcome: Promise<unknown> = Promise.resolve(204)) {
  const send = vi.fn<SendScreenView>(() => outcome);
  const views = createScreenViews(send, () => signals);
  return { send, views };
}

describe("screenRoute", () => {
  it("turns router templates into the server's", () => {
    expect(screenRoute("/")).toBe("/");
    expect(screenRoute("/w/$workspaceId/")).toBe("/w/:workspaceId");
    expect(screenRoute("/w/$workspaceId/inbox/$stream/$conversationId")).toBe(
      "/w/:workspaceId/inbox/:stream/:conversationId",
    );
    expect(screenRoute("/sign-in/passkey")).toBe("/sign-in/passkey");
  });

  it("refuses anything that is not a template", () => {
    for (const path of [
      undefined,
      "",
      `/w/${WORKSPACE}/inbox`,
      "/people/42",
      "/search?q=boiler",
      "/inbox#message-3",
      "/Inbox – Campbooks",
      "/people/maya@northwind-studio.example",
      "/w/$/inbox",
      "inbox",
      `/${"inbox/".repeat(30)}inbox`,
    ]) {
      expect(screenRoute(path), String(path)).toBeUndefined();
    }
  });
});

describe("trackingRefused", () => {
  it("honours Do Not Track and Global Privacy Control", () => {
    expect(trackingRefused({})).toBe(false);
    expect(trackingRefused({ doNotTrack: "0", globalPrivacyControl: false })).toBe(false);
    expect(trackingRefused({ doNotTrack: null })).toBe(false);
    expect(trackingRefused({ doNotTrack: "1" })).toBe(true);
    expect(trackingRefused({ globalPrivacyControl: true })).toBe(true);
  });
});

describe("createScreenViews", () => {
  it("sends the route template, never the address", () => {
    const { send, views } = setup();
    views.viewed("/w/$workspaceId/inbox/$stream/$conversationId");
    expect(send).toHaveBeenCalledExactlyOnceWith("/w/:workspaceId/inbox/:stream/:conversationId");
    expect(JSON.stringify(send.mock.calls)).not.toContain(globalThis.location.href);
  });

  it("sends nothing under Do Not Track", () => {
    const { send, views } = setup({ doNotTrack: "1" });
    views.viewed("/w/$workspaceId/inbox");
    expect(send).not.toHaveBeenCalled();
  });

  it("sends nothing under Global Privacy Control", () => {
    const { send, views } = setup({ globalPrivacyControl: true });
    views.viewed("/w/$workspaceId/inbox");
    expect(send).not.toHaveBeenCalled();
  });

  it("sends nothing for a path that is not a template", () => {
    const { send, views } = setup();
    views.viewed(`/w/${WORKSPACE}/inbox`);
    views.viewed(undefined);
    expect(send).not.toHaveBeenCalled();
  });

  it("drops a failed send without retrying or throwing", async () => {
    const { send, views } = setup({}, Promise.reject(new TypeError("Failed to fetch")));
    expect(() => {
      views.viewed("/w/$workspaceId/inbox");
    }).not.toThrow();
    await Promise.resolve();
    await Promise.resolve();
    expect(send).toHaveBeenCalledOnce();
  });
});

describe("startScreenViews", () => {
  it("reports each navigation to the server once started, and starts once", async () => {
    const mocked = mockApi({ "POST /bff/api/v1/system/screen-views": { status: 204 } });
    const listeners: (() => void)[] = [];
    const navigate = () => {
      for (const listener of listeners) {
        listener();
      }
    };
    let current = "/w/$workspaceId/inbox";
    startScreenViews(
      (changed) => {
        listeners.push(changed);
      },
      () => current,
    );
    startScreenViews(
      () => {
        throw new Error("started twice");
      },
      () => current,
    );

    navigate();
    current = "/w/$workspaceId/settings/members";
    navigate();

    await vi.waitFor(() => {
      expect(mocked.calls).toHaveLength(2);
    });
    expect(mocked.calls.map(({ route, body }) => [route, body])).toEqual([
      ["POST /bff/api/v1/system/screen-views", { route: "/w/:workspaceId/inbox" }],
      ["POST /bff/api/v1/system/screen-views", { route: "/w/:workspaceId/settings/members" }],
    ]);
  });
});
