/**
 * modules/settings/api/use-ai-query — GET /api/app/settings/ai
 *
 * Returns the AI overview: current mode, managed availability,
 * embedding model, BYO adapters, and per-purpose configurations.
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface AiAdapterData {
  id: number;
  name: string;
  provider: string;
  enabled: boolean;
  managed: boolean;
  in_use: boolean;
  endpoint_url: string | null;
  has_api_key: boolean;
}

export interface AiConfigData {
  id: number;
  purpose: string;
  ai_adapter_id: number;
  model: string | null;
}

export interface AiData {
  mode: "managed" | "byo";
  managed_available: boolean;
  embedding_model: string | null;
  adapters: AiAdapterData[];
  configurations: Record<string, AiConfigData>;
}

// ── Query key ─────────────────────────────────────────────────────────────────

export const aiKeys = {
  all: ["settings", "ai"] as const,
};

// ── Query hook ────────────────────────────────────────────────────────────────

export const useAiQuery = (): UseQueryResult<AiData, Error> =>
  useQuery<AiData, Error>({
    queryKey: aiKeys.all,
    queryFn: () => apiClient<AiData>("/settings/ai"),
    staleTime: 2 * 60_000,
  });
