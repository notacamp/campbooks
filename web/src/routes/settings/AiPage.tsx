/**
 * AiPage — Settings → AI at /settings/ai.
 * Stub — not yet implemented. Backed by GET /api/app/settings/ai,
 * GET/PATCH /api/app/settings/ai_adapters, and GET/PATCH /api/app/settings/ai_prompts.
 */
import { type FC } from "react";
import { StubPage } from "./StubPage";

export const AiPage: FC = () => (
  <StubPage
    title="AI"
    description="Configure Scout's AI model, adapters, processing mode, and custom prompts."
  />
);
