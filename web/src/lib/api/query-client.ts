/**
 * lib/api/query-client — TanStack Query client factory.
 *
 * A single QueryClient is created in main.tsx and passed to QueryClientProvider.
 * Modules never instantiate their own clients.
 */
import { QueryClient } from "@tanstack/react-query";
import { ApiError } from "./client";

/** Default stale time: 30 seconds. Adjust per-query as needed. */
const STALE_TIME = 30_000;

/**
 * Creates the app-wide QueryClient with sensible defaults.
 * 401 responses are not retried (the user is not signed in).
 */
export const createQueryClient = (): QueryClient =>
  new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: STALE_TIME,
        retry: (failureCount, error) => {
          if (error instanceof ApiError && error.status === 401) return false;
          return failureCount < 2;
        },
      },
      mutations: {
        retry: false,
      },
    },
  });
