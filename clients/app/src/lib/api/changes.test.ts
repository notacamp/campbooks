import { QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  HIDDEN_GRACE_MS,
  createCacheBinding,
  createWebChangeSource,
  operationKey,
  parseFrame,
  subscribeToChanges,
  type ChangeFrame,
  type ChangeSource,
  type ChangeSourceHandlers,
} from "./changes";

const WS = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c11";
const OTHER_WS = "0192f0c4-7d4e-7b39-a7f1-3a8f0e2b9c22";

/** A source the test drives: records each open and lets it push frames. */
function fakeSource() {
  const opens: { workspace: string; after: string | undefined; closed: boolean }[] = [];
  let handlers: ChangeSourceHandlers | undefined;
  const source: ChangeSource = {
    open(workspace, after, h) {
      const entry = { workspace, after, closed: false };
      opens.push(entry);
      handlers = h;
      return () => {
        entry.closed = true;
      };
    },
  };
  return {
    source,
    opens,
    send(frame: ChangeFrame) {
      handlers?.onFrame(frame);
    },
    refuse() {
      handlers?.onClosed();
    },
  };
}

/** A page whose visibility the test flips. */
function fakePage() {
  const target = new EventTarget();
  const page = {
    visibilityState: "visible" as DocumentVisibilityState,
    addEventListener: target.addEventListener.bind(target),
    removeEventListener: target.removeEventListener.bind(target),
    set(state: DocumentVisibilityState) {
      page.visibilityState = state;
      target.dispatchEvent(new Event("visibilitychange"));
    },
  };
  return page;
}

function change(cursor: string, type: string, id: string, op: "upsert" | "delete" = "upsert") {
  return { kind: "change", cursor, record: { type, id }, op, version: "v" } as const;
}

/** A cache with some queries of both workspaces, all fresh. */
function seededClient() {
  const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity } } });
  const keys = {
    members: operationKey("workspaces.list_members", { workspace_id: WS }),
    otherMembers: operationKey("workspaces.list_members", { workspace_id: OTHER_WS }),
    workspace: operationKey("workspaces.get", { workspace_id: WS }),
    list: operationKey("workspaces.list"),
    health: operationKey("system.health"),
  };
  for (const key of Object.values(keys)) {
    client.setQueryData(key, { cached: true });
  }
  const stale = (key: readonly unknown[]) => client.getQueryState(key)?.isInvalidated ?? "gone";
  return { client, keys, stale };
}

describe("the change feed", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("parseFrame", () => {
    it("reads the frames the server sends and ignores anything else", () => {
      expect(parseFrame("ready", '{"cursor":"4"}')).toStrictEqual({
        kind: "ready",
        cursor: "4",
      });
      expect(parseFrame("revoked", "{}")).toStrictEqual({ kind: "revoked" });
      expect(parseFrame("message", "{}")).toBeUndefined();
      expect(parseFrame("change", "not json")).toBeUndefined();
      expect(parseFrame("change", "42")).toBeUndefined();
    });
  });

  describe("createCacheBinding", () => {
    it("invalidates the queries that read the changed type, in this workspace only", async () => {
      const { client, keys, stale } = seededClient();
      const binding = createCacheBinding(client, WS);

      binding.change({ type: "workspaces.member", id: "u1" }, "upsert");
      binding.change({ type: "workspaces.member", id: "u2" }, "upsert");
      expect(stale(keys.members)).toBe(false);
      await vi.advanceTimersByTimeAsync(50);

      expect(stale(keys.members)).toBe(true);
      expect(stale(keys.otherMembers)).toBe(false);
      expect(stale(keys.workspace)).toBe(false);
      expect(stale(keys.health)).toBe(false);
    });

    it("reaches queries with no workspace that read the type", async () => {
      const { client, keys, stale } = seededClient();
      createCacheBinding(client, WS).change({ type: "workspaces.workspace", id: WS }, "upsert");
      await vi.advanceTimersByTimeAsync(50);
      expect(stale(keys.workspace)).toBe(true);
      expect(stale(keys.list)).toBe(true);
      expect(stale(keys.members)).toBe(false);
    });

    it("drops cached details of a deleted record", async () => {
      const { client, stale } = seededClient();
      const detail = operationKey("workspaces.list_members", { workspace_id: WS, user_id: "u1" });
      client.setQueryData(detail, { cached: true });
      createCacheBinding(client, WS).change({ type: "workspaces.member", id: "u1" }, "delete");
      await vi.advanceTimersByTimeAsync(50);
      expect(stale(detail)).toBe("gone");
    });

    it("re-reads, on a rule's change, the bodies whose images depend on rules", async () => {
      // A rule that trusts a sender shows or hides its images: changed or
      // removed on another device or by another member, the bodies here
      // follow at once (campbooks-cloud#236). The server sends a removal as
      // an upsert of the rule (its state), and a deletion only when it goes.
      const body = (message: string) =>
        operationKey("conversations.get_message_body", {
          workspace_id: WS,
          conversation_id: "c1",
          message_id: message,
          show_images: "false",
        });
      for (const op of ["upsert", "delete"] as const) {
        const { client, stale } = seededClient();
        client.setQueryData(body("trusted"), { html: "", images_trusted: true });
        client.setQueryData(body("held"), { html: "", images_held: "rule_paused" });
        client.setQueryData(body("offered"), { html: "", images_offer: true, images_held: null });
        client.setQueryData(body("plain"), { html: "", images_trusted: false, images_held: null });
        createCacheBinding(client, WS).change({ type: "conversations.rule", id: "r1" }, op);
        await vi.advanceTimersByTimeAsync(50);
        // Bodies with remote-image state are re-read; one without is not.
        expect({ op, trusted: stale(body("trusted")) }).toEqual({ op, trusted: true });
        expect({ op, held: stale(body("held")) }).toEqual({ op, held: true });
        expect({ op, offered: stale(body("offered")) }).toEqual({ op, offered: true });
        expect({ op, plain: stale(body("plain")) }).toEqual({ op, plain: false });
      }

      // A message's own change still re-reads every body of it.
      const { client, stale } = seededClient();
      client.setQueryData(body("plain"), { html: "", images_trusted: false, images_held: null });
      createCacheBinding(client, WS).change(
        { type: "conversations.message", id: "plain" },
        "upsert",
      );
      await vi.advanceTimersByTimeAsync(50);
      expect(stale(body("plain"))).toBe(true);
    });

    it("ignores record types no read operation returns", async () => {
      const { client, keys, stale } = seededClient();
      createCacheBinding(client, WS).change({ type: "nowhere.thing", id: "x" }, "upsert");
      await vi.advanceTimersByTimeAsync(50);
      expect(stale(keys.members)).toBe(false);
    });

    it("invalidates the whole workspace on reset", () => {
      const { client, keys, stale } = seededClient();
      createCacheBinding(client, WS).reset();
      expect(stale(keys.members)).toBe(true);
      expect(stale(keys.workspace)).toBe(true);
      expect(stale(keys.otherMembers)).toBe(false);
    });

    it("flushes what is pending when disposed", () => {
      const { client, keys, stale } = seededClient();
      const binding = createCacheBinding(client, WS);
      binding.change({ type: "workspaces.member", id: "u1" }, "upsert");
      binding.dispose();
      expect(stale(keys.members)).toBe(true);
    });
  });

  describe("subscribeToChanges", () => {
    it("opens the workspace's stream and applies its changes", async () => {
      const feed = fakeSource();
      const { client, keys, stale } = seededClient();
      const stop = subscribeToChanges({
        queryClient: client,
        workspace: WS,
        source: feed.source,
        page: fakePage(),
      });
      expect(feed.opens).toStrictEqual([{ workspace: WS, after: undefined, closed: false }]);

      feed.send({ kind: "ready", cursor: "2" });
      feed.send(change("3", "workspaces.member", "u1"));
      await vi.advanceTimersByTimeAsync(50);
      expect(stale(keys.members)).toBe(true);

      stop();
      expect(feed.opens[0]?.closed).toBe(true);
    });

    it("resumes from the last cursor after the tab was hidden", async () => {
      const feed = fakeSource();
      const page = fakePage();
      const stop = subscribeToChanges({
        queryClient: new QueryClient(),
        workspace: WS,
        source: feed.source,
        page,
      });
      feed.send({ kind: "ready", cursor: "2" });
      feed.send(change("3", "workspaces.member", "u1"));
      feed.send({ kind: "heartbeat", cursor: "5" });

      page.set("hidden");
      await vi.advanceTimersByTimeAsync(HIDDEN_GRACE_MS - 1);
      expect(feed.opens[0]?.closed).toBe(false);
      await vi.advanceTimersByTimeAsync(1);
      expect(feed.opens[0]?.closed).toBe(true);

      page.set("visible");
      expect(feed.opens).toHaveLength(2);
      expect(feed.opens[1]?.after).toBe("5");
      stop();
    });

    it("keeps the stream through a short hide", async () => {
      const feed = fakeSource();
      const page = fakePage();
      const stop = subscribeToChanges({
        queryClient: new QueryClient(),
        workspace: WS,
        source: feed.source,
        page,
      });
      page.set("hidden");
      await vi.advanceTimersByTimeAsync(1000);
      page.set("visible");
      await vi.advanceTimersByTimeAsync(HIDDEN_GRACE_MS);
      expect(feed.opens).toHaveLength(1);
      expect(feed.opens[0]?.closed).toBe(false);
      stop();
    });

    it("waits for a hidden page to show before opening", () => {
      const feed = fakeSource();
      const page = fakePage();
      page.visibilityState = "hidden";
      const stop = subscribeToChanges({
        queryClient: new QueryClient(),
        workspace: WS,
        source: feed.source,
        page,
      });
      expect(feed.opens).toHaveLength(0);
      page.set("visible");
      expect(feed.opens).toHaveLength(1);
      stop();
    });

    it("reopens a refused stream from its cursor, backing off", async () => {
      const feed = fakeSource();
      const { client, keys, stale } = seededClient();
      const stop = subscribeToChanges({
        queryClient: client,
        workspace: WS,
        source: feed.source,
        page: fakePage(),
      });
      feed.send({ kind: "ready", cursor: "7" });
      feed.refuse();
      expect(stale(keys.list)).toBe(true);
      await vi.advanceTimersByTimeAsync(1000);
      expect(feed.opens).toHaveLength(2);
      expect(feed.opens[1]?.after).toBe("7");

      feed.refuse();
      await vi.advanceTimersByTimeAsync(1000);
      expect(feed.opens).toHaveLength(2);
      await vi.advanceTimersByTimeAsync(1000);
      expect(feed.opens).toHaveLength(3);
      stop();
    });

    it("refetches the workspace on reset and continues from its cursor", async () => {
      const feed = fakeSource();
      const page = fakePage();
      const { client, keys, stale } = seededClient();
      const stop = subscribeToChanges({
        queryClient: client,
        workspace: WS,
        source: feed.source,
        page,
      });
      feed.send({ kind: "reset", cursor: "40", reason: "cursor_expired" });
      expect(stale(keys.members)).toBe(true);
      page.set("hidden");
      await vi.advanceTimersByTimeAsync(HIDDEN_GRACE_MS);
      page.set("visible");
      expect(feed.opens[1]?.after).toBe("40");
      stop();
    });

    it("stops for good when the caller loses the workspace", async () => {
      const feed = fakeSource();
      const page = fakePage();
      const { client, keys, stale } = seededClient();
      subscribeToChanges({ queryClient: client, workspace: WS, source: feed.source, page });
      feed.send({ kind: "closing", reason: "max_age" });
      expect(feed.opens[0]?.closed).toBe(false);
      feed.send({ kind: "revoked" });
      expect(feed.opens[0]?.closed).toBe(true);
      expect(stale(keys.list)).toBe(true);
      expect(stale(keys.members)).toBe(true);

      page.set("hidden");
      page.set("visible");
      feed.refuse();
      await vi.advanceTimersByTimeAsync(60_000);
      expect(feed.opens).toHaveLength(1);
    });
  });

  describe("createWebChangeSource", () => {
    /** A stand-in `EventSource` the test controls. */
    class FakeEventSource extends EventTarget {
      static readonly CLOSED = 2;
      readyState = 1;
      closed = false;
      constructor(readonly url: string) {
        super();
      }
      emit(kind: string, data: string) {
        this.dispatchEvent(new MessageEvent(kind, { data }));
      }
      close() {
        this.closed = true;
        this.readyState = FakeEventSource.CLOSED;
      }
    }

    it("follows the BFF's stream and reports frames and refusals", () => {
      vi.stubGlobal("EventSource", FakeEventSource);
      const created: FakeEventSource[] = [];
      const source = createWebChangeSource("https://app.example.com", (url) => {
        const events = new FakeEventSource(url);
        created.push(events);
        return events as unknown as EventSource;
      });
      const frames: ChangeFrame[] = [];
      const onClosed = vi.fn();

      const close = source.open(WS, "12", { onFrame: (f) => frames.push(f), onClosed });
      const events = created[0];
      expect(events?.url).toBe(`https://app.example.com/bff/api/v1/w/${WS}/changes?after=12`);

      events?.emit("change", JSON.stringify(change("13", "workspaces.member", "u1")));
      events?.emit("message", "{}");
      expect(frames).toStrictEqual([{ ...change("13", "workspaces.member", "u1") }]);

      // A dropped connection: EventSource retries on its own.
      events?.dispatchEvent(new Event("error"));
      expect(onClosed).not.toHaveBeenCalled();
      // Refused: it gives up.
      if (events) {
        events.readyState = FakeEventSource.CLOSED;
      }
      events?.dispatchEvent(new Event("error"));
      expect(onClosed).toHaveBeenCalledOnce();

      close();
      expect(events?.closed).toBe(true);
      events?.emit("change", JSON.stringify(change("14", "workspaces.member", "u2")));
      expect(frames).toHaveLength(1);
    });

    it("starts from now without a cursor", () => {
      vi.stubGlobal("EventSource", FakeEventSource);
      const urls: string[] = [];
      const source = createWebChangeSource("https://app.example.com", (url) => {
        urls.push(url);
        return new FakeEventSource(url) as unknown as EventSource;
      });
      source.open(WS, undefined, { onFrame: vi.fn(), onClosed: vi.fn() })();
      expect(urls).toStrictEqual([`https://app.example.com/bff/api/v1/w/${WS}/changes`]);
    });
  });
});
