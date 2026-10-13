/** A non-2xx API response. `problem` is the RFC 9457 problem+json body, if any. */
export class ApiError extends Error {
  readonly status: number;
  readonly problem: unknown;

  constructor(status: number, problem: unknown) {
    super(`API request failed with status ${String(status)}`);
    this.name = "ApiError";
    this.status = status;
    this.problem = problem;
  }
}

/** The request never produced an HTTP response (network down, bridge refused it). */
export class TransportError extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "TransportError";
  }
}

interface ClientResult<TData> {
  data?: TData;
  error?: unknown;
  response: Response;
}

/** Returns the response data of an openapi-fetch call, or throws an {@link ApiError}. */
export function unwrap<TData>(result: ClientResult<TData>): TData {
  if (result.response.ok && result.data !== undefined) {
    return result.data;
  }
  throw new ApiError(result.response.status, result.error);
}

/** Like {@link unwrap} for operations that answer with no body (204). */
export function unwrapEmpty(result: { error?: unknown; response: Response }): void {
  if (!result.response.ok) {
    throw new ApiError(result.response.status, result.error);
  }
}

/**
 * The stable, machine-readable reason of a failed request: the problem's
 * `code` (`last_owner`, `signup_invite_only`, ...), the same in every
 * locale. Branch on this, never on the localized `title` or `detail`.
 * Undefined when the request got no problem document.
 */
export function problemCode(error: unknown): string | undefined {
  if (!(error instanceof ApiError)) {
    return undefined;
  }
  const { problem } = error;
  if (typeof problem === "object" && problem !== null && "code" in problem) {
    return typeof problem.code === "string" ? problem.code : undefined;
  }
  return undefined;
}

/** The HTTP status of a failed request, if it got an answer. */
export function errorStatus(error: unknown): number | undefined {
  return error instanceof ApiError ? error.status : undefined;
}
