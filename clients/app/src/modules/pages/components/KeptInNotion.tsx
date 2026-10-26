import { useTranslation } from "react-i18next";

import { ButtonLink, Icon, Panel, Text } from "~/lib/ui";

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
  const what =
    known === undefined ? humanKind(kind) : t(`notion.kept.kind.${known satisfies KeptKind}`);
  return (
    <Panel variant="dashed" row label={t("notion.kept.title", { what })}>
      <Icon name="document" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <Text as="span" weight="semibold">
          {t("notion.kept.title", { what })}
        </Text>
        <Text as="span" tone="secondary" size="sm">
          {t("notion.kept.body")}
        </Text>
      </div>
      {href === undefined ? null : (
        <ButtonLink variant="secondary" href={href} target="_blank" rel="noopener noreferrer">
          {t("notion.kept.edit")}
        </ButtonLink>
      )}
    </Panel>
  );
}
