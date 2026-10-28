import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, ButtonLink, InlineConfirm, Pill, StatusText } from "~/lib/ui";

import { agoLabel, knownReason, lastSyncAt, notionUrl } from "../notion";
import type { NotionLink, NotionTableLink } from "../types";

export interface NotionSyncProps {
  /** A page's link, or a table's. */
  link: NotionLink | NotionTableLink;
  /** What the link is: a page, or a table (a Notion database). */
  kind?: "page" | "table" | undefined;
  /** Whether the caller may stop the sync (they can write the page). */
  canStop: boolean;
  /** Stop it; whether it was. */
  onStop: () => Promise<boolean>;
  /** Scroll to the first conflict. */
  onShowConflict: () => void;
  /** The time now (tests pin it). */
  now?: Date | undefined;
}

/**
 * What a page linked to Notion says about itself: "Synced with Notion · 1
 * min ago" (or paused, or stopped and why), Open in Notion, Stop syncing
 * (confirmed in place) and, when changes wait for a choice, "N changes to
 * look at" that jumps to the first.
 */
export function NotionSync({
  link,
  kind = "page",
  canStop,
  onStop,
  onShowConflict,
  now,
}: NotionSyncProps) {
  const { t, i18n } = useTranslation("pages");
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  const stopped = link.state === "stopped";
  const reason = knownReason(link.stopped_reason);
  const at = lastSyncAt(link);

  const subject = kind === "table" ? "notion.sync.table" : "notion.sync";
  const address = "notion_page" in link ? link.notion_page : link.data_source;
  let status: string;
  let tone: "ok" | "waiting" | "problem" = "ok";
  if (stopped) {
    status = t(`${subject}.reason.${reason ?? "other"}`);
    tone = "problem";
  } else if (link.state === "paused") {
    status = t("notion.sync.paused");
    tone = "waiting";
  } else {
    status =
      at === undefined
        ? t("notion.sync.waiting")
        : t("notion.sync.badge", { ago: agoLabel(at, now ?? new Date(), i18n.language) });
  }

  return (
    <div className="flex flex-col gap-2" role="group" aria-label={t("notion.sync.label")}>
      <div className="flex flex-wrap items-center gap-2">
        <StatusText tone={tone}>{status}</StatusText>
        {link.conflicts > 0 && !stopped ? (
          <button
            type="button"
            className="cursor-pointer"
            title={t("notion.sync.conflictsHint")}
            onClick={onShowConflict}
          >
            <Pill signal="look">{t("notion.sync.conflicts", { count: link.conflicts })}</Pill>
          </button>
        ) : null}
        <ButtonLink
          variant="ghost"
          href={notionUrl(address)}
          target="_blank"
          rel="noopener noreferrer"
        >
          {t("notion.sync.open")}
        </ButtonLink>
        {!stopped && canStop ? (
          <Button
            variant="ghost"
            aria-expanded={asking}
            onClick={() => {
              setAsking(true);
            }}
          >
            {t("notion.sync.stop")}
          </Button>
        ) : null}
      </div>
      {asking ? (
        <InlineConfirm
          confirmLabel={t("notion.sync.stopConfirm")}
          cancelLabel={t("notion.sync.stopCancel")}
          busy={busy}
          onCancel={() => {
            setAsking(false);
          }}
          onConfirm={() => {
            setBusy(true);
            void onStop().then((done) => {
              setBusy(false);
              if (done) {
                setAsking(false);
              }
            });
          }}
        >
          {t(`${subject}.stopQuestion`)}
        </InlineConfirm>
      ) : null}
    </div>
  );
}
