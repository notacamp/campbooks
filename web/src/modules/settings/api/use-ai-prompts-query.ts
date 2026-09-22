/**
 * modules/settings/api/use-ai-prompts-query
 *
 * GET /api/app/settings/ai_prompts          — full catalog with custom state
 * GET /api/app/settings/ai_prompts/:purpose — one entry
 */
import { useQuery, type UseQueryResult } from "@tanstack/react-query";
import { apiClient } from "~/lib/api";

// ── Type ──────────────────────────────────────────────────────────────────────

export interface AiPromptData {
  purpose: string;
  label: string;
  description: string | null;
  instructions: string | null;
  configured: boolean;
}

// ── Query keys ────────────────────────────────────────────────────────────────

export const aiPromptKeys = {
  all:  ["settings", "ai_prompts"] as const,
  one:  (purpose: string) => ["settings", "ai_prompts", purpose] as const,
};

// ── Hooks ─────────────────────────────────────────────────────────────────────

export const useAiPromptsQuery = (): UseQueryResult<AiPromptData[], Error> =>
  useQuery<AiPromptData[], Error>({
    queryKey: aiPromptKeys.all,
    queryFn: () => apiClient<AiPromptData[]>("/settings/ai_prompts"),
    staleTime: 2 * 60_000,
  });

export const useAiPromptQuery = (
  purpose: string,
): UseQueryResult<AiPromptData, Error> =>
  useQuery<AiPromptData, Error>({
    queryKey: aiPromptKeys.one(purpose),
    queryFn: () => apiClient<AiPromptData>(`/settings/ai_prompts/${purpose}`),
    staleTime: 2 * 60_000,
    enabled: purpose.length > 0,
  });
