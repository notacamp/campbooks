import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as Y from "yjs";

import { fromBase64, toBase64 } from "./bytes";
import { PageSync, type SyncCall, type SyncReply } from "./sync";

/** A server that merges what it is sent, as `pages.sync` does. */
function fakeServer() {
  const doc = new Y.Doc();
  const calls: { update: boolean }[] = [];
  const respond = (body: { state_vector: string; update?: string }): SyncReply => {
    calls.push({ update: body.update !== undefined });
    if (body.update !== undefined) {
      Y.applyUpdate(doc, fromBase64(body.update));
    }
    return {
      update: toBase64(Y.encodeStateAsUpdate(doc, fromBase64(body.state_vector))),
      state_vector: toBase64(Y.encodeStateVector(doc)),
    };
  };
  return { doc, calls, respond };
}

const write = (doc: Y.Doc, text: string) => {
  doc.getText("t").insert(doc.getText("t").length, text);
};

describe("PageSync", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("gathers typing, sends it once, and says saved", async () => {
    const server = fakeServer();
    const doc = new Y.Doc();
    const sync = new PageSync(doc, Y.encodeStateVector(doc), (body) =>
      Promise.resolve(server.respond(body)),
    );
    const states: string[] = [];
    sync.subscribe((state) => states.push(state));
    write(doc, "Hel");
    write(doc, "lo");
    expect(sync.current).toBe("saving");
    await vi.advanceTimersByTimeAsync(500);
    expect(server.calls).toEqual([{ update: true }]);
    expect(server.doc.getText("t").toString()).toBe("Hello");
    expect(sync.current).toBe("saved");
    expect(states).toEqual(["saving", "saved"]);
    sync.stop();
  });

  it("brings in what others wrote without sending it back", async () => {
    const server = fakeServer();
    write(server.doc, "Rui was here. ");
    const doc = new Y.Doc();
    const sync = new PageSync(doc, new Uint8Array([0]), (body) =>
      Promise.resolve(server.respond(body)),
    );
    await sync.pull();
    expect(doc.getText("t").toString()).toBe("Rui was here. ");
    expect(server.calls).toEqual([{ update: false }]);
    write(doc, "Ana too.");
    await vi.advanceTimersByTimeAsync(500);
    expect(server.doc.getText("t").toString()).toBe("Rui was here. Ana too.");
    sync.stop();
  });

  it("keeps trying with a longer wait while the network is down, then catches up", async () => {
    const server = fakeServer();
    let online = false;
    const call: SyncCall = (body) =>
      online ? Promise.resolve(server.respond(body)) : Promise.reject(new TypeError("offline"));
    const doc = new Y.Doc();
    const sync = new PageSync(doc, Y.encodeStateVector(doc), call, {
      retryMs: [1000, 2000],
    });
    write(doc, "Draft");
    await vi.advanceTimersByTimeAsync(500);
    expect(sync.current).toBe("retrying");
    await vi.advanceTimersByTimeAsync(1000);
    expect(sync.current).toBe("retrying");
    online = true;
    await vi.advanceTimersByTimeAsync(2000);
    expect(sync.current).toBe("saved");
    expect(server.doc.getText("t").toString()).toBe("Draft");
    sync.stop();
  });

  it("stops sending once the server refuses a change as invalid", async () => {
    const calls = vi.fn(() => Promise.reject(new Error("refused")));
    const doc = new Y.Doc();
    const sync = new PageSync(doc, Y.encodeStateVector(doc), calls, {
      refused: () => true,
    });
    write(doc, "Bad");
    await vi.advanceTimersByTimeAsync(500);
    expect(sync.current).toBe("rejected");
    write(doc, " more");
    await vi.advanceTimersByTimeAsync(5000);
    expect(calls).toHaveBeenCalledTimes(1);
    sync.stop();
  });

  it("sends what was typed while a call was on its way", async () => {
    const server = fakeServer();
    let release: (() => void) | undefined;
    const call: SyncCall = async (body) => {
      if (release === undefined) {
        await new Promise<void>((resolve) => {
          release = resolve;
        });
      }
      return server.respond(body);
    };
    const doc = new Y.Doc();
    const sync = new PageSync(doc, Y.encodeStateVector(doc), call);
    write(doc, "one");
    await vi.advanceTimersByTimeAsync(500);
    write(doc, " two");
    await vi.advanceTimersByTimeAsync(500);
    release?.();
    await vi.advanceTimersByTimeAsync(1000);
    expect(server.doc.getText("t").toString()).toBe("one two");
    expect(sync.current).toBe("saved");
    sync.stop();
  });
});
