import { invoke as tauriInvoke } from "@tauri-apps/api/core";

import { isTauri } from "~/lib/platform";

import { TransportError } from "./errors";

/**
 * How API requests leave the app. openapi-fetch builds a typed `Request`; the
 * transport decides how it reaches the server. Modules never see this.
 *
 * - web:   same-origin `fetch` to the server's BFF (`/bff/api/v1/...`, and
 *          `/bff/admin/v1/...` for the admin API). The
 *          browser attaches the httpOnly session cookie; JS never sees a token.
 *          Every request carries the BFF's CSRF header, and a `session_required`
 *          answer sends the browser to sign in, coming back to the same page.
 * - tauri: the request is handed to the Rust `api_request` command, which
 *          attaches the token from the OS keychain (ADR 0004).
 */
export interface Transport {
  readonly kind: "web" | "tauri";
  /** Base URL openapi-fetch prefixes to every `/api/v1/...` (and `/admin/v1/...`) path. */
  readonly baseUrl: string;
  readonly fetch: (request: Request) => Promise<Response>;
}

const BFF_PREFIX = "/bff";

/**
 * Header the BFF requires on unsafe requests (with a same-origin request),
 * so a cross-site page cannot forge one. Sent on every request for simplicity.
 */
export const CSRF_HEADER = "X-Campbooks-CSRF";

/** The BFF's problem `code` when a request has no live session. */
const SESSION_REQUIRED = "session_required";

/**
 * The admin API's problem `code` when the last sign-in is too old for it
 * (more than an hour): sign in again with {@link loginUrl}'s `fresh`. Not a
 * signed-out session, so the transport leaves it to the caller.
 */
export const ADMIN_REAUTH_REQUIRED = "admin_reauth_required";

/**
 * Where to start signing in, coming back to `returnTo` (a path on this
 * site). `fresh` makes the person sign in again even with a live session at
 * the identity provider (`max_age=0`), as the admin API asks.
 */
export function loginUrl(returnTo: string, { fresh = false }: { fresh?: boolean } = {}): string {
  const url = `${BFF_PREFIX}/login?return_to=${encodeURIComponent(returnTo)}`;
  return fresh ? `${url}&max_age=0` : url;
}

function uiLanguage(): string {
  return typeof document === "undefined" ? "" : document.documentElement.lang;
}

function currentPage(): string {
  const { pathname, search, hash } = globalThis.location;
  return pathname + search + hash;
}

async function isSessionRequired(response: Response): Promise<boolean> {
  if (response.status !== 401) {
    return false;
  }
  try {
    const problem: unknown = await response.clone().json();
    return (
      typeof problem === "object" &&
      problem !== null &&
      "code" in problem &&
      problem.code === SESSION_REQUIRED
    );
  } catch {
    return false;
  }
}

export function createWebTransport(
  origin: string = globalThis.location.origin,
  send: (request: Request) => Promise<Response> = (request) => globalThis.fetch(request),
  signIn: (url: string) => void = (url) => {
    globalThis.location.assign(url);
  },
): Transport {
  const baseUrl = origin + BFF_PREFIX;
  // Parallel requests can all find the session gone; navigate once.
  let signingIn = false;
  return {
    kind: "web",
    baseUrl,
    fetch: async (request) => {
      const url = new URL(request.url);
      if (url.origin !== origin || !url.pathname.startsWith(`${BFF_PREFIX}/`)) {
        throw new TransportError(`Refusing to send an API request to ${url.href}`);
      }
      // Defense in depth: credentials travel only as the BFF's httpOnly cookie.
      const headers = new Headers(request.headers);
      headers.delete("authorization");
      headers.set(CSRF_HEADER, "1");
      // Problem titles and details come back in the UI's language, which the
      // app mirrors onto <html lang> (it may differ from the browser's).
      const language = uiLanguage();
      if (language !== "") {
        headers.set("accept-language", language);
      }
      const response = await send(
        new Request(request, { headers, credentials: "same-origin", mode: "same-origin" }),
      );
      // The session ended (or never began): sign in and come back here. A 401
      // from the API itself (a token it refused) is left to the caller, so a
      // misconfigured server cannot cause a sign-in loop.
      if (!signingIn && (await isSessionRequired(response))) {
        signingIn = true;
        signIn(loginUrl(currentPage()));
      }
      return response;
    },
  };
}

/** Arguments of the Rust `api_request` command (src-tauri/src/api.rs). */
interface ApiRequestArgs {
  [key: string]: unknown;
  method: string;
  path: string;
  query: [string, string][];
  body: unknown;
}

interface ApiRequestResult {
  status: number;
  body: unknown;
}

export type Invoke = (command: "api_request", args: ApiRequestArgs) => Promise<ApiRequestResult>;

/**
 * Never dialled: `Request` needs an absolute URL, and the Rust side owns the
 * real API origin. Only the path and query of the request are forwarded.
 */
const TAURI_PLACEHOLDER_ORIGIN = "http://api.invalid";

async function readJsonBody(request: Request): Promise<unknown> {
  const text = await request.text();
  if (text === "") {
    return null;
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new TransportError("Only JSON request bodies can cross the native bridge", {
      cause: error,
    });
  }
}

export function createTauriTransport(
  invoke: Invoke = (command, args) => tauriInvoke<ApiRequestResult>(command, args),
): Transport {
  return {
    kind: "tauri",
    baseUrl: TAURI_PLACEHOLDER_ORIGIN,
    fetch: async (request) => {
      const url = new URL(request.url);
      const args: ApiRequestArgs = {
        method: request.method,
        path: url.pathname,
        query: [...url.searchParams.entries()],
        body: await readJsonBody(request),
      };
      let result: ApiRequestResult;
      try {
        result = await invoke("api_request", args);
      } catch (error) {
        throw new TransportError(`Native API bridge failed: ${String(error)}`, { cause: error });
      }
      const body = result.body === null ? null : JSON.stringify(result.body);
      return new Response(body, {
        status: result.status,
        headers: body === null ? {} : { "content-type": "application/json" },
      });
    },
  };
}

export function selectTransport(): Transport {
  return isTauri() ? createTauriTransport() : createWebTransport();
}

/**
 * The transport for background calls nobody is waiting on (the app's own
 * telemetry): like {@link selectTransport}, but an ended session never
 * sends the browser to sign in.
 */
export function selectQuietTransport(): Transport {
  return isTauri()
    ? createTauriTransport()
    : createWebTransport(undefined, undefined, () => {
        // A background call never navigates; the next real request will.
      });
}
