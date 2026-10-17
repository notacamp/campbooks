import {
  infiniteQueryOptions,
  queryOptions,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";

import { adminApi, unwrap, type AdminSchema } from "~/lib/api";

/** What `GET /admin/v1/system/jobs` answers: the queue by queue and kind (admin API). */
export type QueueSummary = AdminSchema<"QueueSummary">;
export type KindSummary = AdminSchema<"KindSummary">;
export type WatchedSummary = AdminSchema<"WatchedSummary">;
export type FailedJob = AdminSchema<"FailedJob">;
export type JobAction = AdminSchema<"JobAction">;
export type RetryKindResult = AdminSchema<"RetryKindResult">;

const PAGE_SIZE = 50;

/** Every jobs query starts with this, so one invalidation refreshes the panel. */
const JOBS_KEY = ["admin", "system.jobs"] as const;

/** The queue's counts. Never retried: a 404 or a sign-in request is an answer. */
export const jobsSummaryQuery = () =>
  queryOptions({
    queryKey: [...JOBS_KEY, "summary"] as const,
    queryFn: async ({ signal }) => unwrap(await adminApi.GET("/admin/v1/system/jobs", { signal })),
    retry: false,
    refetchInterval: 15_000,
  });

/** The dead jobs, newest first, optionally of one kind; paged by cursor. */
export const failedJobsQuery = (kind: string | undefined) =>
  infiniteQueryOptions({
    queryKey: [...JOBS_KEY, "failed", kind ?? null] as const,
    queryFn: async ({ signal, pageParam }) =>
      unwrap(
        await adminApi.GET("/admin/v1/system/jobs/failed", {
          params: {
            query: {
              limit: PAGE_SIZE,
              ...(kind === undefined ? {} : { kind }),
              ...(pageParam === "" ? {} : { after: pageParam }),
            },
          },
          signal,
        }),
      ),
    initialPageParam: "",
    getNextPageParam: (last) => last.next ?? undefined,
    retry: false,
  });

/** Run `mutationFn`, then refresh every jobs query: the counts and the lists changed. */
function useJobsMutation<TVariables, TResult>(
  mutationFn: (variables: TVariables) => Promise<TResult>,
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: JOBS_KEY }),
  });
}

/** Put one dead job back on its queue. Repeating it changes nothing. */
export function useRetryJob() {
  return useJobsMutation(async (id: string): Promise<JobAction> =>
    unwrap(await adminApi.POST("/admin/v1/system/jobs/{id}/retry", { params: { path: { id } } })),
  );
}

/** Delete one dead job. Only a dead job is ever deleted. */
export function useDiscardJob() {
  return useJobsMutation(async (id: string): Promise<JobAction> =>
    unwrap(await adminApi.DELETE("/admin/v1/system/jobs/{id}", { params: { path: { id } } })),
  );
}

/** Retry the dead jobs of one kind (up to 1000 per call). */
export function useRetryKind() {
  return useJobsMutation(async (kind: string): Promise<RetryKindResult> =>
    unwrap(await adminApi.POST("/admin/v1/system/jobs/retry", { body: { kind } })),
  );
}
