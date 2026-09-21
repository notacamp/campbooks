/**
 * MemoryPage — Settings → Scout's memory at /settings/memory.
 * Stub — not yet implemented. Backed by GET /api/app/settings/memory,
 * POST /api/app/settings/memory/teach, and DELETE /api/app/settings/memory/entries/:id.
 */
import { type FC } from "react";
import { StubPage } from "./StubPage";

export const MemoryPage: FC = () => (
  <StubPage
    title="Scout's memory"
    description="See everything Scout has learned from your emails and documents. Teach Scout new rules, or remove habits you no longer want."
  />
);
