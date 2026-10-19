import { describe, expect, it } from "vitest";

import { readEventStream, type EventFrame } from "./event-stream";

/** A body sending `chunks` one after the other. */
function body(chunks: (string | Uint8Array)[]): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder();
  return new ReadableStream({
    start(controller) {
      for (const chunk of chunks) {
        controller.enqueue(typeof chunk === "string" ? encoder.encode(chunk) : chunk);
      }
      controller.close();
    },
  });
}

async function frames(chunks: (string | Uint8Array)[]): Promise<EventFrame[]> {
  const read: EventFrame[] = [];
  for await (const frame of readEventStream(body(chunks))) {
    read.push(frame);
  }
  return read;
}

describe("readEventStream", () => {
  it("reads named frames and skips comments", async () => {
    expect(
      await frames([
        ': keep-alive\n\nevent: thinking\ndata: {"delta":"Hm"}\n\n',
        'event: done\ndata: {"text":"Hi"}\n\n',
      ]),
    ).toEqual([
      { event: "thinking", data: '{"delta":"Hm"}' },
      { event: "done", data: '{"text":"Hi"}' },
    ]);
  });

  it("joins data lines and names an unnamed frame message", async () => {
    expect(await frames(["data: one\ndata: two\n\n"])).toEqual([
      { event: "message", data: "one\ntwo" },
    ]);
  });

  it("accepts every line ending, split anywhere", async () => {
    const text = 'event: text\r\ndata: {"delta":"a"}\r\n\r\nevent: text\rdata: {"delta":"b"}\r\r';
    const whole = await frames([text]);
    expect(whole).toEqual([
      { event: "text", data: '{"delta":"a"}' },
      { event: "text", data: '{"delta":"b"}' },
    ]);
    for (let at = 1; at < text.length; at += 1) {
      expect(await frames([text.slice(0, at), text.slice(at)])).toEqual(whole);
    }
  });

  it("keeps a character split across chunks whole", async () => {
    const bytes = new TextEncoder().encode('data: {"delta":"até"}\n\n');
    // Split inside "é" (two bytes in UTF-8).
    const at = bytes.indexOf(new TextEncoder().encode("é")[0] ?? 0) + 1;
    expect(await frames([bytes.slice(0, at), bytes.slice(at)])).toEqual([
      { event: "message", data: '{"delta":"até"}' },
    ]);
  });

  it("drops a frame the stream never finished", async () => {
    expect(await frames(["event: text\ndata: half"])).toEqual([]);
  });
});
