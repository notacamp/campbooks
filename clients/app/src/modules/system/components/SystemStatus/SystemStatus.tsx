import { useTranslation } from "react-i18next";

import { Banner, Heading, Text } from "~/lib/ui";

import { useSystemHealth, useSystemVersion } from "../../api/queries";

/** Home page: which API build this client talks to, and whether it is healthy. */
export function SystemStatus() {
  const { t } = useTranslation("system");
  const health = useSystemHealth();
  const version = useSystemVersion();

  const versionText = version.isPending
    ? t("checking")
    : version.isError
      ? t("unknown")
      : // `commit` is absent or null when the server doesn't know its build commit.
        version.data.commit
        ? t("versionWithCommit", { version: version.data.version, commit: version.data.commit })
        : version.data.version;

  const healthText = health.isPending
    ? t("checking")
    : health.isError
      ? t("unreachable")
      : t("healthDetail", { status: health.data.status, database: health.data.database });

  return (
    <section aria-labelledby="system-status-title" className="flex max-w-measure flex-col gap-4">
      <Heading level={1} size="page" id="system-status-title">
        {t("title")}
      </Heading>
      <dl className="grid grid-cols-1 gap-x-6 gap-y-2 sm:grid-cols-2">
        <Text as="dt" tone="tertiary">
          {t("apiVersion")}
        </Text>
        <Text as="dd">{versionText}</Text>
        <Text as="dt" tone="tertiary">
          {t("apiHealth")}
        </Text>
        <Text as="dd">{healthText}</Text>
      </dl>
      {health.isError || version.isError ? (
        <Banner tone="danger" role="alert" title={t("errorHint")} />
      ) : null}
    </section>
  );
}
