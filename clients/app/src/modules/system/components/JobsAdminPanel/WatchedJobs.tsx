import { useTranslation } from "react-i18next";

import { List, ListRow, SettingsSection, StatusText } from "~/lib/ui";

import type { WatchedSummary } from "../../api/jobs";

interface WatchedJobsProps {
  watched: readonly WatchedSummary[];
}

/**
 * The background work an operator must hear about when it dies, by what it
 * is for: "Suspensions reaching members' rules: 2 dead deliveries". Until a
 * dead one is retried, what it carried has not happened (a suspended
 * member's rules keep acting).
 */
export function WatchedJobs({ watched }: WatchedJobsProps) {
  const { t } = useTranslation("system");
  if (watched.length === 0) {
    return null;
  }
  const title = t("jobs.watched.title");
  return (
    <SettingsSection title={title} description={t("jobs.watched.lead")}>
      <List label={title}>
        {watched.map((entry) => (
          <ListRow
            key={entry.code}
            title={
              entry.code === "suspension_fanout" ? t("jobs.watched.suspensionFanout") : entry.code
            }
            meta={entry.kinds.join(", ")}
            trailing={
              <StatusText tone={entry.failed === 0 ? "ok" : "problem"}>
                {entry.failed === 0
                  ? t("jobs.watched.none")
                  : t("jobs.watched.dead", { count: entry.failed })}
              </StatusText>
            }
          />
        ))}
      </List>
    </SettingsSection>
  );
}
