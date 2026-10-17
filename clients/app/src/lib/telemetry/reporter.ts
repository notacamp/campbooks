import type { Schema } from "~/lib/api";

import { parseStack } from "./stack";

export type ClientReport = Schema<"ClientReport">;
type ClientException = Schema<"ClientException">;
type ClientMeasurement = Schema<"ClientMeasurement">;

/** What the reporter knows about where it runs, read at each send. */
export interface ReportContext {
  sessionId: string;
  platform: Schema<"ClientPlatform">;
  appVersion: string | undefined;
  /** The route template shown, e.g. `/w/$workspaceId/inbox`. */
  view: () => string | undefined;
  browser: Schema<"ClientBrowser"> | undefined;
}

/** How a batch leaves: the status the server answered. */
export type Send = (report: ClientReport) => Promise<number>;

/** Most items one batch holds (the server keeps 20 of each kind). */
export const MAX_BATCH = 20;

/** How long items wait to be sent together. */
const FLUSH_DELAY_MS = 5000;

/** Longest error message sent; the server keeps only its head anyway. */
const MAX_MESSAGE = 200;

/** Errors that say nothing (a cancelled request, a closed tab). */
const IGNORED = new Set(["AbortError"]);

export interface Reporter {
  error(error: unknown): void;
  measurement(kind: string, values: Record<string, number>): void;
  /** Send what is queued now (the page is being hidden). */
  flush(): void;
}

/**
 * Batches the app's errors and timings and sends them through `send`. A 404
 * means the instance collects nothing: the reporter stops for good. A failed
 * send drops its batch; telemetry never retries or grows without bound.
 */
export function createReporter(
  context: ReportContext,
  send: Send,
  now: () => Date = () => new Date(),
  schedule: (flush: () => void, ms: number) => void = (flush, ms) => {
    globalThis.setTimeout(flush, ms);
  },
): Reporter {
  let stopped = false;
  let scheduled = false;
  let exceptions: ClientException[] = [];
  let measurements: ClientMeasurement[] = [];

  const flush = () => {
    scheduled = false;
    if (stopped || (exceptions.length === 0 && measurements.length === 0)) {
      return;
    }
    const report: ClientReport = {
      session_id: context.sessionId,
      platform: context.platform,
      app_version: context.appVersion ?? null,
      view: context.view() ?? null,
      browser: context.browser ?? null,
      exceptions,
      measurements,
      events: [],
    };
    exceptions = [];
    measurements = [];
    send(report).then(
      (status) => {
        if (status === 404) {
          stopped = true;
        }
      },
      () => {
        // Dropped: telemetry is best effort.
      },
    );
  };

  const later = () => {
    if (!scheduled && !stopped) {
      scheduled = true;
      schedule(flush, FLUSH_DELAY_MS);
    }
  };

  return {
    error(error) {
      if (stopped || exceptions.length >= MAX_BATCH) {
        return;
      }
      const { kind, message, stack } = describe(error);
      if (IGNORED.has(kind)) {
        return;
      }
      exceptions.push({
        kind,
        message: message.slice(0, MAX_MESSAGE),
        timestamp: now().toISOString(),
        frames: parseStack(stack),
      });
      later();
    },
    measurement(kind, values) {
      if (stopped || measurements.length >= MAX_BATCH) {
        return;
      }
      measurements.push({ kind, values, timestamp: now().toISOString() });
      later();
    },
    flush,
  };
}

function describe(error: unknown): { kind: string; message: string; stack: string | undefined } {
  if (error instanceof Error) {
    return { kind: error.name || "Error", message: error.message, stack: error.stack };
  }
  // A thrown string or object: its type only, never its contents.
  return {
    kind: typeof error,
    message: "a value that is not an Error was thrown",
    stack: undefined,
  };
}
