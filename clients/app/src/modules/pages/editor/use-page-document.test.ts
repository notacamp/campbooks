import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import * as Y from "yjs";

import { toBase64 } from "./bytes";
import { usePageDocument } from "./use-page-document";

const server = new Y.Doc();
server.getText("t").insert(0, "Hello");

type SyncFn = (
  page: string,
  body: { state_vector: string; update?: string },
) => Promise<{ revision: number; state_vector: string; update: string }>;

/** A server that has nothing more to say. */
const quiet = () =>
  vi.fn<SyncFn>((_page, body) =>
    Promise.resolve({
      revision: 2,
      state_vector: body.state_vector,
      update: toBase64(Y.encodeStateAsUpdate(new Y.Doc())),
    }),
  );

/** The calls object is the same across renders, as the app memoizes it. */
const callsOf = (sync: Mock<SyncFn>) => ({ sync }) as never;

const input = (calls: never, revision = 1) => ({
  pageId: "p1",
  update: toBase64(Y.encodeStateAsUpdate(server)),
  stateVector: toBase64(Y.encodeStateVector(server)),
  revision,
  calls,
});

describe("a page's document", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("opens with what the server sent, once", () => {
    const sync = quiet();
    const calls = callsOf(sync);
    const { result, rerender } = renderHook((props) => usePageDocument(props), {
      initialProps: input(calls),
    });
    expect(result.current.doc.getText("t").toJSON()).toBe("Hello");
    expect(result.current.state).toBe("saved");
    const first = result.current.doc;
    rerender(input(calls));
    expect(result.current.doc).toBe(first);
  });

  it("sends typing and says saving, then saved when the server answers", async () => {
    const sync = quiet();
    const calls = callsOf(sync);
    const { result } = renderHook(() => usePageDocument(input(calls)));
    act(() => {
      result.current.doc.getText("t").insert(5, "!");
    });
    expect(result.current.state).toBe("saving");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(600);
    });
    expect(sync).toHaveBeenCalledTimes(1);
    expect(sync.mock.calls[0]?.[0]).toBe("p1");
    expect(result.current.state).toBe("saved");
  });

  it("asks for what others wrote when the page's revision moves", async () => {
    const sync = quiet();
    const calls = callsOf(sync);
    const { rerender } = renderHook((props) => usePageDocument(props), {
      initialProps: input(calls, 1),
    });
    expect(sync).not.toHaveBeenCalled();
    rerender(input(calls, 2));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(sync).toHaveBeenCalledTimes(1);
    expect(sync.mock.calls[0]?.[1]).not.toHaveProperty("update");
  });
});
