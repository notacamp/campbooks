import { invoke as tauriInvoke } from "@tauri-apps/api/core";

import { isTauri } from "~/lib/platform";

import { TransportError } from "./errors";

/**
 * How API requests leave the app. openapi-fetch builds a typed `Request`; the
 * transport decides how it reaches the server. Modules never see this.
 *
 * - web:   same-origin `fetch` to the server's BFF (`/bff/api/v1/...`). The
 *          browser attaches the httpOnly session cookie; JS never sees a token.
 * - tauri: the request is handed to the Rust `api_request` command, which
 *          attaches the token from the OS keychain (ADR 0004).
 */
export interface Transport {
  readonly kind: "web" | "tauri";
  /** Base URL openapi-fetch prefixes to every `/api/v1/...` path. */
  readonly baseUrl: string;
  readonly fetch: (request: Request) => Promise<Response>;
}

const BFF_PREFIX = "/bff";

export function createWebTransport(
  origin: string = globalThis.location.origin,
  send: (request: Request) => Promise<Response> = (request) => globalThis.fetch(request),
): Transport {
  const baseUrl = origin + BFF_PREFIX;
  return {
    kind: "web",
    baseUrl,
    fetch: (request) => {
      const url = new URL(request.url);
      if (url.origin !== origin || !url.pathname.startsWith(`${BFF_PREFIX}/`)) {
        return Promise.reject(new TransportError(`Refusing to send an API request to ${url.href}`));
      }
      // Defense in depth: credentials travel only as the BFF's httpOnly cookie.
      const headers = new Headers(request.headers);
      headers.delete("authorization");
      return send(
        new Request(request, { headers, credentials: "same-origin", mode: "same-origin" }),
      );
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
