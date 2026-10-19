// A trusted sender's images never load from a cached body once the trust may
// be gone (campbooks-cloud#236): reopened after a rule changed, or, without
// a live change feed (the desktop and mobile shells), every time it is
// shown, until a fresh read says the rule still holds.
import { screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { LEDGER, LEDGER_DETAIL, ledgerBody, W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";
import { operationKey, refreshBodiesAfterRules } from "~/lib/api";

import { ThreadMessage } from "./ThreadMessage";

/** A fixture list's first item. */
function firstOf<T>(items: readonly T[]): T {
  const [item] = items;
  if (item === undefined) {
    throw new Error("the fixture is empty");
  }
  return item;
}

const MESSAGE = firstOf(LEDGER_DETAIL.messages);
const REMOTE = "https://cdn.weekly-ledger.example/banner.png";
/** The frame's own image policy, images held back and let through. */
const HELD_POLICY = "img-src data:;";
const SHOWN_POLICY = "img-src data: https: http:";
const BODY_ROUTE = `GET ${W}/conversations/${LEDGER.id}/messages/${MESSAGE.id}/body`;
const BODY_KEY = operationKey("conversations.get_message_body", {
  workspace_id: DUARTE.id,
  conversation_id: LEDGER.id,
  message_id: MESSAGE.id,
  show_images: "false",
});

/** The newsletter's body while a rule trusts its sender: its banner is remote. */
const TRUSTED = {
  ...ledgerBody(true),
  html: `<img src="${REMOTE}" alt="The Weekly Ledger" width="560" height="120"><p>Five tax dates</p>`,
  images_trusted: true,
  images_offer: false,
  images_domain: null,
};

/** The same body once the rule is gone: the server holds the banner back. */
const HELD = {
  ...ledgerBody(false),
  html: '<img alt="The Weekly Ledger" width="560" height="120"><p>Five tax dates</p>',
};

/** The body endpoint, answering only once the test lets it. */
function gatedBody(answer: unknown) {
  let release: (() => void) | undefined;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  mockApi({ [BODY_ROUTE]: () => ({ body: answer }) });
  const answered = globalThis.fetch;
  const asked = { count: 0 };
  globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    asked.count += 1;
    await gate;
    return answered(input, init);
  };
  return {
    asked,
    release: () => {
      release?.();
    },
  };
}

function message(open: boolean) {
  return (
    <ThreadMessage
      workspaceId={DUARTE.id}
      conversationId={LEDGER.id}
      message={MESSAGE}
      open={open}
      onOpen={vi.fn()}
      showImages={false}
      onShowImages={vi.fn()}
      kind="service"
    />
  );
}

/** Every frame's document, joined: what the reader handed the frame to load. */
function frames(): string {
  return [...document.querySelectorAll("iframe")]
    .map((frame) => frame.getAttribute("srcdoc") ?? "")
    .join("\n");
}

describe("a trusted sender's images (campbooks-cloud#236)", () => {
  it("never load from the cache once a rule changed: a reopened message waits for its fresh read", async () => {
    // The web follows the change feed live.
    vi.stubGlobal("EventSource", vi.fn());
    const body = gatedBody(HELD);
    const { queryClient, rerender } = renderWithProviders(message(false));
    // Read while the rule trusted the sender, then folded.
    queryClient.setQueryData(BODY_KEY, TRUSTED);

    // The rule is removed (here, or anywhere through its live change).
    await refreshBodiesAfterRules(queryClient, DUARTE.id);
    rerender(message(true));

    // Before the read answers: nothing of the cached body, no remote source.
    await waitFor(() => {
      expect(document.querySelector('[aria-busy="true"]')).not.toBeNull();
    });
    expect(document.querySelector("iframe")).toBeNull();
    await waitFor(() => {
      expect(body.asked.count).toBe(1);
    });
    expect(document.querySelector("iframe")).toBeNull();

    body.release();
    await waitFor(() => {
      expect(frames()).toContain("Five tax dates");
    });
    expect(frames()).not.toContain(REMOTE);
    expect(frames()).toContain(HELD_POLICY);
    expect(screen.getByText(/1 remote image hidden/u)).toBeVisible();
  });

  it("stay held back, without a live feed, while a cached body is read again", async () => {
    // No `EventSource` here, as in the desktop and mobile shells.
    const body = gatedBody(HELD);
    const { queryClient, rerender } = renderWithProviders(message(false));
    // Read a while ago, while a rule trusted the sender; removed elsewhere since.
    queryClient.setQueryData(BODY_KEY, TRUSTED, { updatedAt: Date.now() - 1000 });
    rerender(message(true));

    // The cached body shows at once, its banner's source taken out.
    await waitFor(() => {
      expect(frames()).toContain("Five tax dates");
    });
    expect(frames()).not.toContain(REMOTE);
    expect(frames()).toContain(HELD_POLICY);
    expect(screen.queryByText(/you trust The Weekly Ledger/u)).toBeNull();
    await waitFor(() => {
      expect(body.asked.count).toBe(1);
    });

    body.release();
    expect(await screen.findByText(/1 remote image hidden/u)).toBeVisible();
    expect(frames()).not.toContain(REMOTE);
  });

  it("load, without a live feed, once the fresh read says the rule still holds", async () => {
    const body = gatedBody(TRUSTED);
    const { queryClient, rerender } = renderWithProviders(message(false));
    queryClient.setQueryData(BODY_KEY, TRUSTED, { updatedAt: Date.now() - 1000 });
    rerender(message(true));
    await waitFor(() => {
      expect(frames()).toContain("Five tax dates");
    });
    expect(frames()).not.toContain(REMOTE);

    body.release();
    await waitFor(() => {
      expect(frames()).toContain(REMOTE);
    });
    expect(frames()).toContain(SHOWN_POLICY);
    expect(
      screen.getByText("Images shown: you trust The Weekly Ledger. Tracking pixels stay removed."),
    ).toBeVisible();
  });
});
