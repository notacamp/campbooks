import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, EmptyState, Heading, InlineConfirm, List, ListRow, Text } from "~/lib/ui";

import { useRetryKind, type KindSummary, type QueueSummary } from "../../api/jobs";
import { formatUptime } from "../SystemAdminPanel/SystemOverview";

import type { Notice } from "./notice";

export interface JobsSummaryProps {
  summary: QueueSummary;
  /** Narrow the failed list to this kind. */
  onShowFailed: (kind: string) => void;
  onNotice: (notice: Notice) => void;
}

/** The queue, one row per queue and kind: what waits, runs and failed. */
export function JobsSummary({ summary, onShowFailed, onNotice }: JobsSummaryProps) {
  const { t } = useTranslation("system");
  const [confirming, setConfirming] = useState<string | null>(null);
  const retryKind = useRetryKind();

  const retryAll = (kind: string) => {
    retryKind.mutate(kind, {
      onSuccess: (result) => {
        setConfirming(null);
        onNotice({
          tone: "neutral",
          text:
            result.requeued === 0
              ? t("jobs.retryAll.none", { kind })
              : t("jobs.retryAll.done", { count: result.requeued, kind }) +
                (result.more ? ` ${t("jobs.retryAll.more")}` : ""),
        });
      },
      onError: () => {
        setConfirming(null);
        onNotice({ tone: "danger", text: t("jobs.retryAll.error", { kind }) });
      },
    });
  };

  return (
    <section className="flex flex-col gap-3">
      <Heading level={2} size="section">
        {t("jobs.queue.title")}
      </Heading>
      {summary.kinds.length === 0 ? (
        <EmptyState title={t("jobs.queue.emptyTitle")}>{t("jobs.queue.emptyLead")}</EmptyState>
      ) : (
        <List label={t("jobs.queue.title")}>
          {summary.kinds.map((entry) => {
            const key = `${entry.queue}/${entry.kind}`;
            return (
              <ListRow
                key={key}
                title={entry.kind}
                badge={entry.queue}
                trailing={
                  entry.failed === 0 ? undefined : (
                    <>
                      <Button
                        variant="ghost"
                        onClick={() => {
                          onShowFailed(entry.kind);
                        }}
                      >
                        {t("jobs.queue.showFailed")}
                      </Button>
                      <Button
                        variant="secondary"
                        onClick={() => {
                          setConfirming(key);
                        }}
                      >
                        {t("jobs.queue.retryAll")}
                      </Button>
                    </>
                  )
                }
              >
                <Counts entry={entry} />
                {confirming === key ? (
                  <InlineConfirm
                    confirmLabel={t("jobs.retryAll.confirm")}
                    cancelLabel={t("jobs.cancel")}
                    busy={retryKind.isPending}
                    onConfirm={() => {
                      retryAll(entry.kind);
                    }}
                    onCancel={() => {
                      setConfirming(null);
                    }}
                  >
                    {t("jobs.retryAll.question", { count: entry.failed, kind: entry.kind })}
                  </InlineConfirm>
                ) : undefined}
              </ListRow>
            );
          })}
        </List>
      )}
    </section>
  );
}

/** "Ready 3 · Scheduled 1 · Running 2 · Failed 4", plus what is worth a look. */
function Counts({ entry }: { entry: KindSummary }) {
  const { t, i18n } = useTranslation("system");
  const waiting =
    entry.ready > 0 &&
    entry.oldest_ready_seconds !== undefined &&
    entry.oldest_ready_seconds !== null
      ? (formatUptime(entry.oldest_ready_seconds, i18n.language) ?? t("jobs.queue.underAMinute"))
      : undefined;
  return (
    <Text tone="secondary" size="sm">
      {t("jobs.queue.counts", {
        ready: entry.ready,
        scheduled: entry.scheduled,
        running: entry.running,
        failed: entry.failed,
      })}
      {waiting === undefined ? null : ` · ${t("jobs.queue.oldestWaiting", { age: waiting })}`}
      {entry.expired_leases === 0
        ? null
        : ` · ${t("jobs.queue.expiredLeases", { count: entry.expired_leases })}`}
    </Text>
  );
}
