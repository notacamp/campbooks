import { queryOptions, useQuery } from "@tanstack/react-query";

import { api, operationKey, unwrap } from "~/lib/api";

const systemQueries = {
  health: () =>
    queryOptions({
      queryKey: operationKey("system.health"),
      queryFn: async ({ signal }) => unwrap(await api.GET("/api/v1/system/health", { signal })),
    }),
  version: () =>
    queryOptions({
      queryKey: operationKey("system.version"),
      queryFn: async ({ signal }) => unwrap(await api.GET("/api/v1/system/version", { signal })),
      staleTime: Number.POSITIVE_INFINITY,
    }),
};

export function useSystemHealth() {
  return useQuery(systemQueries.health());
}

export function useSystemVersion() {
  return useQuery(systemQueries.version());
}
