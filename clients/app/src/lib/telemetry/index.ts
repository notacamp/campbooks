import { onCLS, onFCP, onINP, onLCP, onTTFB, type Metric } from "web-vitals";

import { backgroundApi, type Schema } from "~/lib/api";
import { isTauri } from "~/lib/platform";

import { createReporter, type ClientReport, type Reporter } from "./reporter";

/**
 * The app's own telemetry (`openspec/specs/observability`): errors it did
 * not handle and its web vitals, sent in batches to the server
 * (`system.report_client_telemetry`), which scrubs them and forwards them to
 * the operator's Grafana Faro receiver. Nothing about the page's address,
 * the person, their mail or their workspace is sent: the route template
 * shown, a random id for this run of the app, the browser's name and
 * platform, and for an error its class, message and stack positions. When
 * the instance collects nothing the server answers 404 and sending stops.
 */
let reporter: Reporter | undefined;

/** Report an error the app did not handle (a no-op until started). */
export function reportError(error: unknown): void {
  reporter?.error(error);
}

/** Start reporting, once. `currentView` names the route template shown. */
export function startTelemetry(currentView: () => string | undefined): void {
  if (reporter !== undefined) {
    return;
  }
  const started = createReporter(
    {
      sessionId: randomId(),
      platform: isTauri() ? "native" : "web",
      appVersion: undefined,
      view: currentView,
      browser: browser(),
    },
    send,
  );
  reporter = started;

  globalThis.addEventListener("error", (event) => {
    started.error(event.error);
  });
  globalThis.addEventListener("unhandledrejection", (event) => {
    started.error(event.reason);
  });
  const vital = (metric: Metric) => {
    started.measurement("web-vitals", {
      [metric.name.toLowerCase()]: metric.value,
      delta: metric.delta,
    });
  };
  onCLS(vital);
  onFCP(vital);
  onINP(vital);
  onLCP(vital);
  onTTFB(vital);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") {
      started.flush();
    }
  });
}

async function send(report: ClientReport): Promise<number> {
  const { response } = await backgroundApi.POST("/api/v1/system/client-telemetry", {
    body: report,
  });
  return response.status;
}

/** Twelve random letters and digits: one run of the app, nothing more. */
function randomId(): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  const bytes = crypto.getRandomValues(new Uint8Array(12));
  return Array.from(bytes, (byte) => alphabet[byte % alphabet.length]).join("");
}

interface UserAgentData {
  brands?: { brand: string; version: string }[];
  mobile?: boolean;
  platform?: string;
}

/** The browser's name and platform where the browser says them plainly. */
function browser(): Schema<"ClientBrowser"> | undefined {
  const data = (navigator as Navigator & { userAgentData?: UserAgentData }).userAgentData;
  if (data === undefined) {
    return undefined;
  }
  const brand = data.brands?.find(({ brand }) => !brand.includes("Not"));
  return {
    name: brand?.brand ?? null,
    version: brand?.version ?? null,
    os: data.platform ?? null,
    mobile: data.mobile ?? null,
  };
}
