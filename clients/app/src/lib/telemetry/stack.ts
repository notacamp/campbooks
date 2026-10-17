import type { Schema } from "~/lib/api";

export type Frame = Schema<"ClientFrame">;

/** Most frames kept from a stack (the server keeps 30). */
export const MAX_FRAMES = 30;

// `    at fn (https://…/a.js:12:34)` and `    at https://…/a.js:12:34` (V8).
const V8_FRAME = /^\s*at (?:(.+?) \()?(.+?):(\d+):(\d+)\)?$/;
// `fn@https://…/a.js:12:34` (WebKit, Gecko).
const AT_FRAME = /^(.*?)@(.+?):(\d+):(\d+)$/;

/**
 * The frames of an error's `stack`, innermost first. Only the script, the
 * function and the position: the server keeps the script's file name and
 * nothing else of its address.
 */
export function parseStack(stack?: string): Frame[] {
  if (stack === undefined) {
    return [];
  }
  const frames: Frame[] = [];
  for (const line of stack.split("\n")) {
    const match = V8_FRAME.exec(line) ?? AT_FRAME.exec(line);
    if (match === null) {
      continue;
    }
    const [, fn, file, lineNo, column] = match;
    frames.push({
      file: file ?? null,
      function: fn === undefined || fn === "" ? null : fn,
      line: Number(lineNo),
      column: Number(column),
    });
    if (frames.length === MAX_FRAMES) {
      break;
    }
  }
  return frames;
}
