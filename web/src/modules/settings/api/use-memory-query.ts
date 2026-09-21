/**
 * modules/settings/api/use-memory-query — GET /api/app/settings/memory
 *
 * Returns the Scout memory catalog: a list of behaviour sentences derived
 * on demand from all the workspace's inbox rules, learned habits, tags, etc.
 * Optionally filtered by a ?facet= from Scout::Memory::Entry::FACETS.
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";

// ── Types ─────────────────────────────────────────────────────────────────────

export type MemoryOrigin = "taught" | "learned" | "default";
export type MemoryFacet =
  | "filing"
  | "stack"
  | "replies"
  | "streams"
  | "people"
  | "style"
  | "automations";

export type MemoryAction = "edit" | "remove" | "confirm";

export interface MemorySentence {
  plain: string;
  html: string;
}

export interface MemoryEntryData {
  id: string;
  facet: MemoryFacet;
  sentence: string | MemorySentence;
  origin: MemoryOrigin;
  origin_detail: string | null;
  actions: MemoryAction[];
  form_path: string | null;
}

export interface MemoryCatalogData {
  entries: MemoryEntryData[];
  facet_counts: Record<string, number>;
  total: number;
}

// ── Query keys ────────────────────────────────────────────────────────────────

export const memoryKeys = {
  all:    ["settings", "memory"] as const,
  facet:  (facet: MemoryFacet | null) =>
    facet ? (["settings", "memory", facet] as const) : memoryKeys.all,
};

// ── Hook ──────────────────────────────────────────────────────────────────────

export const useMemoryQuery = (
  facet: MemoryFacet | null = null,
): UseQueryResult<MemoryCatalogData, Error> => {
  const path = facet
    ? `/settings/memory?facet=${facet}`
    : "/settings/memory";

  return useQuery<MemoryCatalogData, Error>({
    queryKey: memoryKeys.facet(facet),
    queryFn:  () => apiClient<MemoryCatalogData>(path),
    staleTime: 60_000,
  });
};
