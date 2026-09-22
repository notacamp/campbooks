/**
 * modules/settings/api/use-ai-prompts-mutation
 *
 * PATCH /api/app/settings/ai_prompts/:purpose — update (or clear) a prompt.
 * Clearing instructions (empty string) resets to the built-in default.
 */
import {
  useMutation,
  useQueryClient,
  type UseMutationResult,
} from "@tanstack/react-query";
import { apiClient } from "~/lib/api";
import type { AiPromptData } from "./use-ai-prompts-query";
import { aiPromptKeys } from "./use-ai-prompts-query";

export interface UpdatePromptParams {
  purpose: string;
  instructions: string;
}

export const useUpdatePromptMutation = (): UseMutationResult<
  AiPromptData,
  Error,
  UpdatePromptParams
> => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ purpose, instructions }: UpdatePromptParams) =>
      apiClient<AiPromptData>(`/settings/ai_prompts/${purpose}`, {
        method: "PATCH",
        body: { ai_prompt: { instructions } },
      }),
    onSuccess: (data) => {
      queryClient.setQueryData(aiPromptKeys.one(data.purpose), data);
      // Invalidate the full list so the catalog badge refreshes
      void queryClient.invalidateQueries({ queryKey: aiPromptKeys.all });
    },
  });
};
