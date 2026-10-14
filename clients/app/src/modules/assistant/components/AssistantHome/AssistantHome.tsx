import { useTranslation } from "react-i18next";

import { Heading, Text } from "~/lib/ui";

/** Placeholder home page of the `assistant` module. */
export function AssistantHome() {
  const { t } = useTranslation("assistant");

  return (
    <section aria-labelledby="assistant-home-title" className="flex max-w-measure flex-col gap-4 p-6">
      <Heading level={1} size="page" id="assistant-home-title">
        {t("title")}
      </Heading>
      <Text tone="secondary">{t("summary")}</Text>
    </section>
  );
}
