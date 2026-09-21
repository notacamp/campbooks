/**
 * lib/api/client — fetch primitive for the Campbooks /api/app surface.
 *
 * Every call attaches the bearer token from the token stub and unwraps the
 * three envelope shapes the Rails API returns:
 *   { data }              — single resource
 *   { data, meta }        — paginated collection
 *   { error: { code, message } } — API-level error
 *
 * HTTP-level errors (4xx/5xx) are surfaced as ApiError instances so TanStack
 * Query can distinguish them from network failures.
 *
 * Modules consume this through their own api/ hooks (useQuery / useMutation),
 * NEVER by importing this file directly. Direct imports are restricted to
 * lib/api by the ESLint no-restricted-imports rule.
 */
import { getToken } from "./token";
import { logger } from "~/lib/logger";

// ── Envelope types ────────────────────────────────────────────────────────────

export interface ApiMeta {
  page?: number;
  totalPages?: number;
  totalCount?: number;
  [key: string]: unknown;
}

export interface ApiSuccessEnvelope<T> {
  data: T;
  meta?: ApiMeta;
}

export interface ApiErrorEnvelope {
  error: {
    code: string;
    message: string;
  };
}

export type ApiEnvelope<T> = ApiSuccessEnvelope<T> | ApiErrorEnvelope;

// ── Error class ───────────────────────────────────────────────────────────────

export class ApiError extends Error {
  readonly code: string;
  readonly status: number;

  constructor(code: string, message: string, status: number) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.status = status;
  }
}

// ── Core fetcher ──────────────────────────────────────────────────────────────

const BASE_URL = "/api/app";

export interface FetchOptions extends Omit<RequestInit, "body"> {
  body?: unknown;
}

/**
 * Typed fetch primitive for /api/app endpoints.
 * Returns the unwrapped `data` value on success.
 * Throws ApiError on 4xx/5xx or when the body contains { error }.
 */
export const apiClient = async <T>(
  path: string,
  options: FetchOptions = {},
): Promise<T> => {
  const token = getToken();
  const { body, ...rest } = options;

  const response = await fetch(`${BASE_URL}${path}`, {
    ...rest,
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...rest.headers,
    },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
  });

  const json = (await response.json()) as ApiEnvelope<T>;

  if ("error" in json) {
    const { code, message } = json.error;
    logger.warn(`API error [${response.status}] ${code}: ${message}`);
    throw new ApiError(code, message, response.status);
  }

  if (!response.ok) {
    throw new ApiError("http_error", response.statusText, response.status);
  }

  return json.data;
};

// ── Collection meta (paginated responses) ─────────────────────────────────────

/**
 * Meta returned by paginated Rails endpoints (pagy page-based).
 * Keys match the Rails API wire format (snake_case).
 */
export interface CollectionMeta {
  page: number;
  per_page: number;
  total: number;
  total_pages: number;
}

export interface CollectionEnvelope<T> {
  data: T[];
  meta: CollectionMeta;
}

/**
 * Typed fetch primitive for paginated /api/app collection endpoints.
 * Returns `{ data, meta }` so callers can drive infinite scroll.
 * Throws ApiError on 4xx/5xx or when the body contains { error }.
 *
 * Use this (via a module's api/ hook) for list endpoints.
 * Use `apiClient` for single-resource endpoints.
 */
export const apiCollection = async <T>(
  path: string,
  options: FetchOptions = {},
): Promise<CollectionEnvelope<T>> => {
  const token = getToken();
  const { body, ...rest } = options;

  const response = await fetch(`${BASE_URL}${path}`, {
    ...rest,
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...rest.headers,
    },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
  });

  // 204 No Content — return an empty collection.
  if (response.status === 204) {
    return { data: [], meta: { page: 1, per_page: 0, total: 0, total_pages: 1 } };
  }

  const json = (await response.json()) as ApiEnvelope<T[]>;

  if ("error" in json) {
    const { code, message } = json.error;
    logger.warn(`API error [${response.status}] ${code}: ${message}`);
    throw new ApiError(code, message, response.status);
  }

  if (!response.ok) {
    throw new ApiError("http_error", response.statusText, response.status);
  }

  const meta = (json.meta as unknown as CollectionMeta) ?? {
    page: 1,
    per_page: 30,
    total: 0,
    total_pages: 1,
  };

  return { data: json.data, meta };
};
