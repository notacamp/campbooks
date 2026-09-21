/**
 * routes/inbox/utils — pure helpers for the Inbox route.
 */

/**
 * Returns a human-readable relative time string for an ISO timestamp.
 * Examples: "just now", "5m", "2h", "Mon", "Mar 12".
 */
export const formatRelativeTime = (isoString: string): string => {
  const now = Date.now();
  const then = new Date(isoString).getTime();
  if (Number.isNaN(then)) return "";

  const diffMs = now - then;
  const diffMin = Math.floor(diffMs / 60_000);

  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin}m`;

  const diffHours = Math.floor(diffMin / 60);
  if (diffHours < 24) return `${diffHours}h`;

  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 7) {
    const d = new Date(then);
    return d.toLocaleDateString("en", { weekday: "short" });
  }

  const d = new Date(then);
  return d.toLocaleDateString("en", { month: "short", day: "numeric" });
};
