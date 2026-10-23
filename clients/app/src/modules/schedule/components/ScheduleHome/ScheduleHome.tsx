import { useTranslation } from "react-i18next";

import { Heading, Text } from "~/lib/ui";

/** Placeholder home page of the `schedule` module. */
export function ScheduleHome() {
  const { t } = useTranslation("schedule");

  return (
    <section
      aria-labelledby="schedule-home-title"
      className="flex max-w-measure flex-col gap-4 p-6"
    >
      <Heading level={1} size="page" id="schedule-home-title">
        {t("title")}
      </Heading>
      <Text tone="secondary">{t("summary")}</Text>
    </section>
  );
}
