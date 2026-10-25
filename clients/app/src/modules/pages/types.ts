import type { IconName } from "~/lib/ui";
import type { Schema } from "~/lib/api";

export type PageSummary = Schema<"PageSummary">;
export type PageView = Schema<"PageView">;
export type PageList = Schema<"PageList">;
export type PageRole = Schema<"PageRole">;
export type PagePlace = Schema<"Place">;
export type GrantView = Schema<"GrantView">;
export type Backlinks = Schema<"Backlinks">;
export type SyncResponse = Schema<"SyncResponse">;
export type UploadedFile = Schema<"UploadedFile">;
export type NewPage = Schema<"NewPage">;
export type PageMoveBody = Schema<"PageMove">;

/** The icons a page may have: the server's fixed set, by the design system's icon names. */
export const PAGE_ICONS = [
  "document",
  "tasks",
  "schedule",
  "people",
  "star",
  "pin",
  "mail",
  "money",
  "home",
  "inbox",
  "archive",
  "image",
  "travel",
  "dining",
  "parcel",
  "ticket",
] as const satisfies readonly IconName[];

export type PageIcon = (typeof PAGE_ICONS)[number];

/** The page's icon as the design system draws it. */
export function iconOf(icon: string | null | undefined): PageIcon {
  return PAGE_ICONS.find((name) => name === icon) ?? "document";
}

/** The record types a page may mention (the server's list). */
export const MENTION_TYPES = [
  "workspaces.member",
  "conversations.conversation",
  "tasks.task",
  "schedule.event",
  "pages.page",
  "person",
] as const;

export type MentionType = (typeof MENTION_TYPES)[number];

export function isMentionType(value: unknown): value is MentionType {
  return (MENTION_TYPES as readonly unknown[]).includes(value);
}
