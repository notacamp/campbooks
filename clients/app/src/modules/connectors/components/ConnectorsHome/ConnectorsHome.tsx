import { useTranslation } from "react-i18next";

/** Placeholder home page of the `connectors` module. */
export function ConnectorsHome() {
  const { t } = useTranslation("connectors");

  return (
    <section aria-labelledby="connectors-home-title" className="max-w-xl space-y-4">
      <h1 id="connectors-home-title" className="text-2xl font-semibold">
        {t("title")}
      </h1>
      <p className="text-muted">{t("summary")}</p>
    </section>
  );
}
