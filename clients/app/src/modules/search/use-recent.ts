import { useState } from "react";

import { useSession } from "~/lib/api";

import { forgetSearches, recentKey, recentSearches, rememberSearch } from "./model";

/**
 * The member's recent searches on this device, for the signed-in account and
 * workspace. Nothing is read or stored until the session says who is signed in.
 */
export function useRecentSearches(workspaceId: string) {
  const account = useSession().data?.user?.sub;
  const key = recentKey(account, workspaceId);
  // Clearing re-renders so the list is read again (storage is the source).
  const [, setCleared] = useState(0);
  return {
    items: recentSearches(key),
    remember: (text: string) => {
      rememberSearch(key, text);
    },
    clear: () => {
      forgetSearches(key);
      setCleared((count) => count + 1);
    },
  };
}
