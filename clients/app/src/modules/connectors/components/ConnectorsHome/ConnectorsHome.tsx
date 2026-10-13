import { useTranslation } from "react-i18next";

import { Heading, Text } from "~/lib/ui";

/** Placeholder home page of the `connectors` module. */
export function ConnectorsHome() {
  const { t } = useTranslation("connectors");

  return (
    <section aria-labelledby="connectors-home-title" className="max-w-xl space-y-4">
      <Heading level={1} size="page" id="connectors-home-title">
        {t("title")}
      </Heading>
      <Text tone="tertiary">{t("summary")}</Text>
    </section>
  );
}
