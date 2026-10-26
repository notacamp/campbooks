import { useTranslation } from "react-i18next";

import { Icon } from "~/lib/ui";

import { humanKind, KEPT_KINDS, type KeptKind } from "../notion-kinds";

export interface KeptInNotionProps {
  /** The block's Notion type (`column_list`, `synced_block`, ...), as the server wrote it. */
  kind: string;
  /** Where to edit it: the Notion page; none when the page is not linked. */
  href?: string | undefined;
}

/**
 * A block Pages doesn't have yet: shown, never edited from here, and kept
 * exactly as it is in Notion ("Kept in Notion · Columns").
 */
export function KeptInNotion({ kind, href }: KeptInNotionProps) {
  const { t } = useTranslation("pages");
  const known = KEPT_KINDS.find((candidate) => candidate === kind);
  const what = known === undefined ? humanKind(kind) : t(`notion.kept.kind.${known satisfies KeptKind}`);
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-md border border-dashed border-strong bg-surface-sunken p-3">
      <Icon name="document" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <strong className="break-words">{t("notion.kept.title", { what })}</strong>
        <span className="text-sm text-secondary">{t("notion.kept.body")}</span>
      </div>
      {href === undefined ? null : (
        <a
          className="text-sm underline"
          href={href}
          target="_blank"
          rel="noopener noreferrer"
        >
          {t("notion.kept.edit")}
        </a>
      )}
    </div>
  );
}
