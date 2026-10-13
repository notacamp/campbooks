// Where a recording may be made. Recordings are published (PR media is
// world-readable), so they are made against a local stack with demo data
// only: never against a deployed instance with real people's mail.

/** Host names a recording may target without `--allow-remote`. */
const LOCAL_HOSTS: readonly string[] = ["localhost", "127.0.0.1", "[::1]"];

/**
 * Why `baseUrl` may not be recorded, or `undefined` when it may: it must be
 * an http(s) URL on this machine, unless the caller passed `--allow-remote`.
 */
export function refuseTarget(baseUrl: string, allowRemote: boolean): string | undefined {
  let url: URL;
  try {
    url = new URL(baseUrl);
  } catch {
    return `--base-url ${baseUrl} is not a URL`;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return `--base-url ${baseUrl} is not an http(s) URL`;
  }
  if (allowRemote || LOCAL_HOSTS.includes(url.hostname)) return undefined;
  return (
    `refusing to record ${url.origin}: recordings are made against this machine only ` +
    `(${LOCAL_HOSTS.join(", ")}), with demo data, never against a deployed instance. ` +
    "Pass --allow-remote only for a disposable instance holding no real mail."
  );
}
