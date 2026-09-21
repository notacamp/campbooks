/**
 * modules/settings/api/use-members-query — GET /api/app/settings/members
 *
 * Returns the list of workspace members + pending invitations.
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { MembersData } from "~/modules/settings/types";

export const membersKeys = {
  all: ["settings", "members"] as const,
};

export const useMembersQuery = (): UseQueryResult<MembersData, Error> =>
  useQuery<MembersData, Error>({
    queryKey: membersKeys.all,
    queryFn: () => apiClient<MembersData>("/settings/members"),
    staleTime: 2 * 60_000,
  });
