import { useQuery } from "@tanstack/react-query";

import { ADMIN_REAUTH_REQUIRED, errorStatus, problemCode } from "~/lib/api";
import { adminOverviewQuery } from "~/modules/system";

/**
 * Whether the signed-in person may use the admin section (ADR 0015):
 *
 * - `checking`: asking the admin API;
 * - `admin`: its system overview answered;
 * - `none`: it answered 404, as it does for everyone who is not an
 *   administrator, so nothing admin is shown;
 * - `reauth`: an administrator whose sign-in is too old; sign in again;
 * - `error`: the server did not answer.
 */
export type AdminAccess = "checking" | "admin" | "none" | "reauth" | "error";

/** Reads {@link AdminAccess} from the system overview (never retried). */
export function useAdminAccess(): { access: AdminAccess; retry: () => void } {
  const overview = useQuery(adminOverviewQuery());
  const retry = () => {
    void overview.refetch();
  };
  if (overview.isPending) {
    return { access: "checking", retry };
  }
  if (overview.isSuccess) {
    return { access: "admin", retry };
  }
  if (problemCode(overview.error) === ADMIN_REAUTH_REQUIRED) {
    return { access: "reauth", retry };
  }
  return { access: errorStatus(overview.error) === 404 ? "none" : "error", retry };
}
