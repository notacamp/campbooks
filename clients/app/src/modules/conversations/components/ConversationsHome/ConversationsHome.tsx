import { useTranslation } from "react-i18next";

import { Heading, Text } from "~/lib/ui";

/** Placeholder home page of the `conversations` module. */
export function ConversationsHome() {
  const { t } = useTranslation("conversations");

  return (
    <section
      aria-labelledby="conversations-home-title"
      className="flex max-w-measure flex-col gap-4 p-6"
    >
      <Heading level={1} size="page" id="conversations-home-title">
        {t("title")}
      </Heading>
      <Text tone="tertiary">{t("summary")}</Text>
    </section>
  );
}
