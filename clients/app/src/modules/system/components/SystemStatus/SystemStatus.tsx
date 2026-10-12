import { useTranslation } from "react-i18next";

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
    <section aria-labelledby="system-status-title" className="max-w-xl space-y-4">
      <h1 id="system-status-title" className="text-2xl font-semibold">
        {t("title")}
      </h1>
      <dl className="grid grid-cols-1 gap-x-6 gap-y-2 sm:grid-cols-[max-content_1fr]">
        <dt className="text-muted">{t("apiVersion")}</dt>
        <dd>{versionText}</dd>
        <dt className="text-muted">{t("apiHealth")}</dt>
        <dd>{healthText}</dd>
      </dl>
      {health.isError || version.isError ? (
        <p role="alert" className="text-danger">
          {t("errorHint")}
        </p>
      ) : null}
    </section>
  );
}
