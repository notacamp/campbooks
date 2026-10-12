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
