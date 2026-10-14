import type { ReactNode } from "react";

import { useSuspendedAccounts } from "../../api/admin";

export interface SuspendedAccountsProps {
  /** The accounts on the page (the `sub` of their tokens). */
  accountIds: readonly string[];
  /** What to show, knowing which of them are suspended. */
  children: (suspended: ReadonlySet<string>) => ReactNode;
}

/**
 * Admin → People: which of the accounts listed are suspended, for the
 * workspaces module's list (one request per page of people).
 */
export function SuspendedAccounts({ accountIds, children }: SuspendedAccountsProps) {
  return children(useSuspendedAccounts(accountIds));
}
