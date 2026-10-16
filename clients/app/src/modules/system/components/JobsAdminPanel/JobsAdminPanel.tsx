import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Banner, Button, EmptyState, SettingsPage, SkeletonList } from "~/lib/ui";

import { jobsSummaryQuery } from "../../api/jobs";

import { FailedJobs } from "./FailedJobs";
import { JobsSummary } from "./JobsSummary";
import type { Notice } from "./notice";

/**
 * Admin → Jobs: the background queue by kind, and the jobs that gave up,
 * with their last error, to retry or discard. Never shows what a job
 * carries: only what ran, how often, and why it stopped.
 */
export function JobsAdminPanel() {
  const { t } = useTranslation("system");
  const summary = useQuery(jobsSummaryQuery());
  const [kind, setKind] = useState<string>();
  const [notice, setNotice] = useState<Notice | null>(null);

  return (
    <SettingsPage
      label={t("jobs.title")}
      title={t("jobs.title")}
      lead={t("jobs.lead")}
      actions={
        <Button
          variant="ghost"
          disabled={summary.isFetching}
          onClick={() => {
            void summary.refetch();
          }}
        >
          {t("jobs.refresh")}
        </Button>
      }
    >
      {notice === null ? null : (
        <Banner
          tone={notice.tone}
          icon={notice.tone === "danger" ? "warning" : "check"}
          title={notice.text}
          role={notice.tone === "danger" ? "alert" : "status"}
          action={
            <Button
              variant="ghost"
              onClick={() => {
                setNotice(null);
              }}
            >
              {t("jobs.dismiss")}
            </Button>
          }
        />
      )}
      {summary.isPending ? (
        <div role="status" aria-label={t("jobs.loading")}>
          <SkeletonList rows={4} />
        </div>
      ) : summary.isError ? (
        <EmptyState
          title={t("jobs.loadError.title")}
          visor="thinking"
          visorSize="lg"
          action={
            <Button
              onClick={() => {
                void summary.refetch();
              }}
            >
              {t("jobs.loadError.retry")}
            </Button>
          }
        >
          {t("jobs.loadError.lead")}
        </EmptyState>
      ) : (
        <div className="flex flex-col gap-5">
          <JobsSummary summary={summary.data} onShowFailed={setKind} onNotice={setNotice} />
          <FailedJobs
            kind={kind}
            kinds={summary.data.kinds
              .filter((entry) => entry.failed > 0)
              .map((entry) => entry.kind)}
            onKind={setKind}
            onNotice={setNotice}
          />
        </div>
      )}
    </SettingsPage>
  );
}
