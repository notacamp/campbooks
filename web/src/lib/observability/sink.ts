/**
 * lib/observability — error-tracking and performance sink stub.
 *
 * This is where an error-tracking service (e.g. Sentry) would be wired as a
 * logger sink. The pattern: the sink registers itself via registerLogSink() so
 * lib/logger never imports from lib/observability (lib/ arrows point inward).
 *
 * Nothing in the codebase imports @sentry/* or any other tracking SDK except
 * files inside lib/observability. That import restriction is enforced by ESLint.
 *
 * To add real error-tracking:
 *   1. Install the SDK (e.g. @sentry/react).
 *   2. Implement the sink below.
 *   3. Call initObservability() from main.tsx after loading runtime config.
 */
import { registerLogSink } from "~/lib/logger";
import type { LogEvent } from "~/lib/logger";

/** Stub sink — replace with real SDK calls when an error-tracking service is configured. */
const observabilitySink = (event: LogEvent): void => {
  // TODO: forward event.level === "error" to an error-tracking service.
  // Example (Sentry):
  //   if (event.level === "error") Sentry.captureMessage(event.message);
  void event; // suppress unused-variable lint until the stub is filled in
};

/** Wire the observability sink to the logger. Call once from main.tsx. */
export const initObservability = (): void => {
  registerLogSink(observabilitySink);
};
