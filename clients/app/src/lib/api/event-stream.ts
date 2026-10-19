/**
 * Server-Sent Events read from a `fetch` body, for streams `EventSource`
 * cannot open: a POST with a JSON body (a streamed reply draft,
 * campbooks-cloud#288). Lines end with `\r\n`, `\n` or `\r`; comments are
 * skipped; `data` lines are joined with `\n`; a blank line ends a frame.
 * Only `event` and `data` are read.
 */

/** One frame: its `event` name (`message` when none) and its `data`. */
export interface EventFrame {
  event: string;
  data: string;
}

/** The frames of `body`, as they arrive. */
export async function* readEventStream(
  body: ReadableStream<Uint8Array>,
): AsyncGenerator<EventFrame, void, undefined> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let event = "";
  let data: string[] = [];
  // A `\r` at the end of a chunk may be the first half of `\r\n`.
  let lastWasCr = false;

  const line = (text: string): EventFrame | undefined => {
    if (text === "") {
      const frame =
        data.length > 0
          ? { event: event === "" ? "message" : event, data: data.join("\n") }
          : undefined;
      event = "";
      data = [];
      return frame;
    }
    if (text.startsWith(":")) {
      return undefined;
    }
    const colon = text.indexOf(":");
    const field = colon === -1 ? text : text.slice(0, colon);
    let value = colon === -1 ? "" : text.slice(colon + 1);
    if (value.startsWith(" ")) {
      value = value.slice(1);
    }
    if (field === "event") {
      event = value;
    } else if (field === "data") {
      data.push(value);
    }
    return undefined;
  };

  try {
    for (;;) {
      const { done, value } = await reader.read();
      buffer += done ? decoder.decode() : decoder.decode(value, { stream: true });
      if (lastWasCr && buffer.startsWith("\n")) {
        buffer = buffer.slice(1);
      }
      lastWasCr = false;
      for (;;) {
        const end = buffer.search(/[\r\n]/u);
        if (end === -1) {
          break;
        }
        const text = buffer.slice(0, end);
        const cr = buffer[end] === "\r";
        let rest = buffer.slice(end + 1);
        if (cr) {
          if (rest.startsWith("\n")) {
            rest = rest.slice(1);
          } else if (rest === "") {
            lastWasCr = true;
          }
        }
        buffer = rest;
        const frame = line(text);
        if (frame !== undefined) {
          yield frame;
        }
      }
      if (done) {
        return;
      }
    }
  } finally {
    reader.releaseLock();
  }
}
