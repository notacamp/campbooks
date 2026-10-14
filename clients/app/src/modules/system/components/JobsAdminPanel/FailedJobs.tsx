import { useInfiniteQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  CodeBlock,
  EmptyState,
  Heading,
  InlineConfirm,
  List,
  ListRow,
  Select,
  SkeletonList,
  Text,
} from "~/lib/ui";

import {
  failedJobsQuery,
  useDiscardJob,
  useRetryJob,
  type FailedJob,
  type JobAction,
} from "../../api/jobs";

import type { Notice } from "./notice";

export interface FailedJobsProps {
  /** Only this kind (`undefined`: all). */
  kind: string | undefined;
  /** The kinds that have failures, for the filter. */
  kinds: readonly string[];
  onKind: (kind: string | undefined) => void;
  onNotice: (notice: Notice) => void;
}

/** The jobs that gave up, newest first, with their last error. */
export function FailedJobs({ kind, kinds, onKind, onNotice }: FailedJobsProps) {
  const { t } = useTranslation("system");
  const failed = useInfiniteQuery(failedJobsQuery(kind));
  const [discarding, setDiscarding] = useState<string | null>(null);
  const retry = useRetryJob();
  const discard = useDiscardJob();
  const items = failed.data?.pages.flatMap((page) => page.items) ?? [];
  const options = [...new Set([...(kind === undefined ? [] : [kind]), ...kinds])].toSorted();

  const said = (action: JobAction, applied: string, unchanged: string): Notice => ({
    tone: "neutral",
    text:
      action.outcome === "applied"
        ? applied
        : action.outcome === "superseded"
          ? t("jobs.failed.superseded")
          : unchanged,
  });

  return (
    <section className="flex flex-col gap-3">
      <Heading level={2} size="section">
        {t("jobs.failed.title")}
      </Heading>
      <Select
        label={t("jobs.failed.filter")}
        value={kind ?? ""}
        options={[
          { value: "", label: t("jobs.failed.allKinds") },
          ...options.map((option) => ({ value: option, label: option })),
        ]}
        onChange={(value) => {
          onKind(value === "" ? undefined : value);
        }}
      />
      {failed.isPending ? (
        <div role="status" aria-label={t("jobs.loading")}>
          <SkeletonList rows={3} />
        </div>
      ) : failed.isError ? (
        <EmptyState
          title={t("jobs.loadError.title")}
          visor="thinking"
          visorSize="lg"
          action={
            <Button
              onClick={() => {
                void failed.refetch();
              }}
            >
              {t("jobs.loadError.retry")}
            </Button>
          }
        >
          {t("jobs.loadError.lead")}
        </EmptyState>
      ) : items.length === 0 ? (
        <EmptyState title={t("jobs.failed.emptyTitle")}>{t("jobs.failed.emptyLead")}</EmptyState>
      ) : (
        <>
          <List label={t("jobs.failed.title")}>
            {items.map((job) => (
              <FailedRow
                key={job.id}
                job={job}
                busy={retry.isPending || discard.isPending}
                discarding={discarding === job.id}
                onRetry={() => {
                  retry.mutate(job.id, {
                    onSuccess: (action) => {
                      onNotice(said(action, t("jobs.failed.retried"), t("jobs.failed.unchanged")));
                    },
                    onError: () => {
                      onNotice({ tone: "danger", text: t("jobs.failed.retryError") });
                    },
                  });
                }}
                onAskDiscard={() => {
                  setDiscarding(job.id);
                }}
                onCancelDiscard={() => {
                  setDiscarding(null);
                }}
                onDiscard={() => {
                  discard.mutate(job.id, {
                    onSuccess: (action) => {
                      setDiscarding(null);
                      onNotice(
                        said(action, t("jobs.failed.discarded"), t("jobs.failed.unchanged")),
                      );
                    },
                    onError: () => {
                      setDiscarding(null);
                      onNotice({ tone: "danger", text: t("jobs.failed.discardError") });
                    },
                  });
                }}
              />
            ))}
          </List>
          {failed.hasNextPage ? (
            <Button
              variant="secondary"
              disabled={failed.isFetchingNextPage}
              onClick={() => {
                void failed.fetchNextPage();
              }}
            >
              {t("jobs.failed.more")}
            </Button>
          ) : null}
        </>
      )}
    </section>
  );
}

interface FailedRowProps {
  job: FailedJob;
  busy: boolean;
  discarding: boolean;
  onRetry: () => void;
  onAskDiscard: () => void;
  onCancelDiscard: () => void;
  onDiscard: () => void;
}

function FailedRow({
  job,
  busy,
  discarding,
  onRetry,
  onAskDiscard,
  onCancelDiscard,
  onDiscard,
}: FailedRowProps) {
  const { t, i18n } = useTranslation("system");
  const when =
    job.finished_at === undefined || job.finished_at === null
      ? undefined
      : new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium", timeStyle: "short" }).format(
          new Date(job.finished_at),
        );
  const detail = [
    t("jobs.failed.attempts", { attempts: job.attempts, max: job.max_attempts }),
    job.workspace_id === undefined || job.workspace_id === null
      ? t("jobs.failed.instanceWide")
      : t("jobs.failed.workspace", { workspace: job.workspace_id }),
  ].join(" · ");

  return (
    <ListRow
      title={job.kind}
      badge={job.queue}
      meta={when === undefined ? undefined : t("jobs.failed.when", { when })}
      trailing={
        <>
          <Button variant="secondary" disabled={busy} onClick={onRetry}>
            {t("jobs.failed.retry")}
          </Button>
          <Button variant="ghost" disabled={busy} onClick={onAskDiscard}>
            {t("jobs.failed.discard")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        <Text tone="secondary" size="sm">
          {detail}
        </Text>
        {job.last_error === undefined || job.last_error === null || job.last_error === "" ? (
          <Text tone="tertiary">{t("jobs.failed.noError")}</Text>
        ) : (
          <CodeBlock
            label={t("jobs.failed.error")}
            copyLabel={t("jobs.failed.copy")}
            copiedLabel={t("jobs.failed.copied")}
            failedLabel={t("jobs.failed.copyFailed")}
          >
            {job.last_error}
          </CodeBlock>
        )}
        {discarding ? (
          <InlineConfirm
            confirmLabel={t("jobs.failed.discardConfirm")}
            cancelLabel={t("jobs.cancel")}
            busy={busy}
            onConfirm={onDiscard}
            onCancel={onCancelDiscard}
          >
            {t("jobs.failed.discardQuestion", { kind: job.kind })}
          </InlineConfirm>
        ) : null}
      </div>
    </ListRow>
  );
}
