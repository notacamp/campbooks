/**
 * lib/api/token — bearer token storage stub.
 *
 * Campbooks uses server-issued tokens for the /api/app surface. The real token
 * flow (POST /api/app/session → exchange Rails session for a bearer token) is
 * NOT implemented here — that belongs to the auth module and is owned by
 * campbooks-e1 (the API session owner).
 *
 * This stub reads/writes to sessionStorage as a placeholder so the API client
 * has something to call. Replace with the real store once the auth module lands.
 */

const TOKEN_KEY = "campbooks_api_token";

/** Returns the stored bearer token, or null if none exists. */
export const getToken = (): string | null => {
  try {
    return sessionStorage.getItem(TOKEN_KEY);
  } catch {
    // sessionStorage may throw in private browsing or sandboxed iframes.
    return null;
  }
};

/** Stores a bearer token. Call after a successful POST /api/app/session. */
export const setToken = (token: string): void => {
  try {
    sessionStorage.setItem(TOKEN_KEY, token);
  } catch {
    // Silently ignore storage errors.
  }
};

/** Clears the stored token (call on sign-out). */
export const clearToken = (): void => {
  try {
    sessionStorage.removeItem(TOKEN_KEY);
  } catch {
    // Silently ignore.
  }
};
