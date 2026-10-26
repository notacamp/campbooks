import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button, Icon, Pill } from "~/lib/ui";

import type { NotionKeep } from "../types";

export interface ConflictBlockProps {
  /** Notion's version of the block, shown under ours (the editor's content); none when Notion removed it. */
  children?: ReactNode;
  /** Notion removed the block while it was edited here. */
  removed: boolean;
  /** Settle it; whether it was. */
  onChoose: (keep: NotionKeep) => Promise<boolean>;
  /** Read-only: someone who cannot write the page sees the conflict but cannot settle it. */
  readOnly?: boolean | undefined;
}

const CHOICES = ["mine", "notion", "both"] as const satisfies readonly NotionKeep[];

/**
 * A block that changed in Notion while it was edited here: Notion's version
 * is shown under ours, with Keep mine, Use Notion's and Keep both. Neither
 * goes to Notion until one is chosen (the page says it changed in Notion;
 * it cannot say who). Different paragraphs merge on their own; only the same
 * one asks.
 */
export function ConflictBlock({
  children,
  removed,
  onChoose,
  readOnly = false,
}: ConflictBlockProps) {
  const { t } = useTranslation("pages");
  const [busy, setBusy] = useState<NotionKeep | undefined>();
  const choose = (keep: NotionKeep) => {
    setBusy(keep);
    void onChoose(keep).then(() => {
      setBusy(undefined);
    });
  };
  return (
    <section
      className="flex flex-col gap-2 rounded-md border border-strong bg-surface-sunken p-3"
      aria-label={t("notion.conflict.label")}
    >
      <div className="flex flex-wrap items-center gap-2">
        <Icon name="warning" />
        <Pill signal="look">
          {t(removed ? "notion.conflict.removed" : "notion.conflict.changed")}
        </Pill>
      </div>
      {removed ? (
        <p className="text-sm text-secondary">{t("notion.conflict.removedBody")}</p>
      ) : (
        <div className="flex flex-col gap-1">
          <span className="text-sm text-secondary">{t("notion.conflict.theirs")}</span>
          <div contentEditable={false} suppressContentEditableWarning>
            {children}
          </div>
        </div>
      )}
      {readOnly ? null : (
        <div
          className="flex flex-wrap gap-2"
          role="group"
          aria-label={t("notion.conflict.choices")}
        >
          {CHOICES.map((keep) => (
            <Button
              key={keep}
              variant={keep === "mine" ? "primary" : "secondary"}
              disabled={busy !== undefined}
              onClick={() => {
                choose(keep);
              }}
            >
              {t(`notion.conflict.${keep}`)}
            </Button>
          ))}
        </div>
      )}
      <p className="text-xs text-tertiary">{t("notion.conflict.footer")}</p>
    </section>
  );
}
