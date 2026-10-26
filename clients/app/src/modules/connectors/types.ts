import type { Schema } from "~/lib/api";

// The Notion shapes of the connectors API, named once here so a rename in the
// generated document is a one-line fix.
export type StartNotion = Schema<"StartNotion">;
export type StartedNotion = Schema<"StartedNotion">;
export type NotionPages = Schema<"NotionPages">;
export type NotionPage = Schema<"NotionPage">;
