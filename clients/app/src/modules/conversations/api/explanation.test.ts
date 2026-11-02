import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { POLL_LIMIT_MS, pollDelay } from "./explanation";

describe("pollDelay", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-11-02T10:00:00Z"));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("asks every 1.5 s while it is written, and stops 90 s after it was first seen so", () => {
    const seen = new Map<string, number>();
    expect(pollDelay("writing", "a", Date.now(), seen)).toBe(1500);
    // Each fetch renews the data's own time; the count does not move.
    vi.advanceTimersByTime(POLL_LIMIT_MS - 1500);
    expect(pollDelay("writing", "a", Date.now(), seen)).toBe(1500);
    vi.advanceTimersByTime(1500);
    expect(pollDelay("writing", "a", Date.now(), seen)).toBe(false);
    vi.advanceTimersByTime(60_000);
    expect(pollDelay("writing", "a", Date.now(), seen)).toBe(false);
  });

  it("starts again for a new request, and never polls once it is settled", () => {
    const seen = new Map<string, number>();
    pollDelay("writing", "a", Date.now(), seen);
    vi.advanceTimersByTime(POLL_LIMIT_MS + 1);
    expect(pollDelay("ready", "a", Date.now(), seen)).toBe(false);
    expect(pollDelay("writing", "a", Date.now(), seen)).toBe(1500);
  });
});
