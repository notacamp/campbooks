import { describe, expect, it, vi } from "vitest";

import { createReporter, MAX_BATCH, type ClientReport, type ReportContext } from "./reporter";

const context: ReportContext = {
  sessionId: "k3Jx8vQ2mN",
  platform: "web",
  appVersion: undefined,
  view: () => "/w/$workspaceId/inbox",
  browser: undefined,
};

function setup(status = 202) {
  const sent: ClientReport[] = [];
  const send = vi.fn((report: ClientReport) => {
    sent.push(report);
    return Promise.resolve(status);
  });
  let pending: (() => void) | undefined;
  const reporter = createReporter(
    context,
    send,
    () => new Date("2026-10-17T10:00:00Z"),
    (flush) => {
      pending = flush;
    },
  );
  const tick = async () => {
    pending?.();
    pending = undefined;
    await Promise.resolve();
  };
  return { reporter, sent, send, tick };
}

describe("createReporter", () => {
  it("batches errors and timings with the route template, never the address", async () => {
    const { reporter, sent, tick } = setup();
    reporter.error(new TypeError("Cannot read properties of undefined"));
    reporter.measurement("web-vitals", { lcp: 1234.5, delta: 1234.5 });
    await tick();

    expect(sent).toHaveLength(1);
    const [report] = sent;
    expect(report?.view).toBe("/w/$workspaceId/inbox");
    expect(report?.session_id).toBe("k3Jx8vQ2mN");
    expect(report?.exceptions?.[0]).toMatchObject({
      kind: "TypeError",
      message: "Cannot read properties of undefined",
      timestamp: "2026-10-17T10:00:00.000Z",
    });
    expect(report?.measurements?.[0]?.values).toEqual({ lcp: 1234.5, delta: 1234.5 });
    expect(JSON.stringify(report)).not.toContain(globalThis.location.href);
  });

  it("sends nothing of a value thrown that is not an Error", async () => {
    const { reporter, sent, tick } = setup();
    reporter.error({ subject: "Q4 restructuring plan", to: "maya@example.com" });
    await tick();
    const json = JSON.stringify(sent);
    expect(json).not.toContain("Q4 restructuring plan");
    expect(json).not.toContain("maya@example.com");
    expect(sent[0]?.exceptions?.[0]?.kind).toBe("object");
  });

  it("caps a batch and ignores aborted requests", async () => {
    const { reporter, sent, tick } = setup();
    for (let i = 0; i < MAX_BATCH + 5; i += 1) {
      reporter.error(new Error("boom"));
    }
    reporter.error(new DOMException("The user aborted a request.", "AbortError"));
    await tick();
    expect(sent[0]?.exceptions).toHaveLength(MAX_BATCH);
  });

  it("stops for good when the instance collects nothing", async () => {
    const { reporter, send, tick } = setup(404);
    reporter.error(new Error("first"));
    await tick();
    await Promise.resolve();
    reporter.error(new Error("second"));
    reporter.flush();
    await tick();
    expect(send).toHaveBeenCalledTimes(1);
  });
});
