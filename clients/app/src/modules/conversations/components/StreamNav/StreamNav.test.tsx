import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from "@tanstack/react-router";
import { screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { STREAM_COUNTS, W } from "../../../../../test/demo/conversations";
import { DUARTE } from "../../../../../test/demo/workspaces";
import { mockApi, renderWithProviders } from "../../../../../test/render";
import type { Place } from "../../api/queries";

import { StreamChips } from "./StreamNav";

function chips(place: Place) {
  const root = createRootRoute({ component: Outlet });
  const page = createRoute({
    getParentRoute: () => root,
    path: "/w/$workspaceId/inbox/$stream",
    component: () => <StreamChips workspaceId={DUARTE.id} current={place} />,
  });
  const router = createRouter({
    routeTree: root.addChildren([page]),
    history: createMemoryHistory({ initialEntries: [`/w/${DUARTE.id}/inbox/${place}`] }),
  });
  return renderWithProviders(<RouterProvider router={router} />);
}

describe("the phone chips", () => {
  const scrolled = vi.fn();
  let calls: Element[] = [];

  beforeEach(() => {
    calls = [];
    scrolled.mockReset();
    Element.prototype.scrollIntoView = function (this: Element, options?: unknown) {
      calls.push(this);
      scrolled(options);
    };
    mockApi({ [`GET ${W}/streams`]: { body: { streams: STREAM_COUNTS } } });
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    Reflect.deleteProperty(Element.prototype, "scrollIntoView");
  });

  it("lists the streams, then Sent, Pinned, Drafts, Archive, Spam and Trash", async () => {
    chips("primary");
    const row = await screen.findByRole("group", { name: "Streams" });
    const names = [...row.querySelectorAll("button")].map((chip) => chip.firstChild?.textContent);
    expect(names).toEqual([
      "Primary",
      "Updates",
      "Receipts",
      "Newsletters",
      "Sent",
      "Pinned",
      "Drafts",
      "Archive",
      "Spam",
      "Trash",
    ]);
  });

  it("brings the chip of the place you are in into view, smoothly", async () => {
    chips("trash");
    const trash = await screen.findByRole("button", { name: "Trash" });
    await waitFor(() => {
      expect(scrolled).toHaveBeenCalled();
    });
    expect(calls[0]).toBe(trash);
    expect(scrolled).toHaveBeenCalledWith({
      inline: "nearest",
      block: "nearest",
      behavior: "smooth",
    });
  });

  it("does not animate under reduced motion", async () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query.includes("prefers-reduced-motion"),
      media: query,
    }));
    chips("drafts");
    await waitFor(() => {
      expect(scrolled).toHaveBeenCalledWith({
        inline: "nearest",
        block: "nearest",
        behavior: "auto",
      });
    });
  });
});
