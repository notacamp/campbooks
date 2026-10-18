import { backgroundApi } from "~/lib/api";

/**
 * Usage analytics (`openspec/specs/usage-analytics`): after each navigation
 * to another path, the app tells its server which screen it shows, as the
 * route's template with `:name` parameters (`/w/:workspaceId/inbox`), never
 * the address, an id, the search, the fragment or the page title. The server
 * relays it to the operator's Matomo when the instance records usage
 * analytics and drops it otherwise; the app loads no analytics script.
 * Nothing is sent while the browser asks not to be tracked (Do Not Track or
 * Global Privacy Control). Fire and forget: a failed send is never retried.
 */

/** Longest route template the server takes. */
const MAX_ROUTE = 120;

/** `/`, or segments that are lowercase words or `:name` parameters. */
const ROUTE = /^(?:\/|(?:\/(?:[a-z][a-z_-]*|:[a-z][A-Za-z]*))+)$/;

/** The browser's requests not to be tracked. */
export interface PrivacySignals {
  doNotTrack?: string | null;
  globalPrivacyControl?: boolean;
}

/** How a screen view leaves: never awaited for its answer. */
export type SendScreenView = (route: string) => Promise<unknown>;

/**
 * A router template (`/w/$workspaceId/inbox/`) as the server takes it
 * (`/w/:workspaceId/inbox`), or `undefined` when it is not one.
 */
export function screenRoute(fullPath: string | undefined): string | undefined {
  if (fullPath === undefined) {
    return undefined;
  }
  const trimmed = fullPath.length > 1 && fullPath.endsWith("/") ? fullPath.slice(0, -1) : fullPath;
  const route = trimmed
    .split("/")
    .map((segment) => (segment.startsWith("$") ? `:${segment.slice(1)}` : segment))
    .join("/");
  return route.length <= MAX_ROUTE && ROUTE.test(route) ? route : undefined;
}

/** Whether the browser asks not to be tracked. */
export function trackingRefused(signals: PrivacySignals): boolean {
  return signals.doNotTrack === "1" || signals.globalPrivacyControl === true;
}

interface ScreenViews {
  /** The screen whose router template is `fullPath` is shown. */
  viewed(fullPath: string | undefined): void;
}

/** Screen views sent through `send`, unless `signals` refuse tracking. */
export function createScreenViews(
  send: SendScreenView,
  signals: () => PrivacySignals = () => navigator,
): ScreenViews {
  return {
    viewed(fullPath) {
      if (trackingRefused(signals())) {
        return;
      }
      const route = screenRoute(fullPath);
      if (route === undefined) {
        return;
      }
      send(route).catch(() => {
        // Dropped: usage analytics is best effort.
      });
    },
  };
}

let started = false;

/**
 * Start reporting screen views, once. `subscribe` calls back after each
 * navigation to another path; `currentRoute` names the router template shown.
 */
export function startScreenViews(
  subscribe: (changed: () => void) => void,
  currentRoute: () => string | undefined,
): void {
  if (started) {
    return;
  }
  started = true;
  const views = createScreenViews(sendScreenView);
  subscribe(() => {
    views.viewed(currentRoute());
  });
}

async function sendScreenView(route: string): Promise<void> {
  await backgroundApi.POST("/api/v1/system/screen-views", { body: { route } });
}
