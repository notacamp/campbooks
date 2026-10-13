import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { Heading, Text } from "~/lib/ui";

export function NotFound() {
  const { t } = useTranslation();
  return (
    <section className="flex flex-col gap-2">
      <Heading level={1} size="title">
        {t("notFound.title")}
      </Heading>
      <Text tone="secondary">{t("notFound.body")}</Text>
      <Link to="/" className="font-semibold text-accent">
        {t("notFound.home")}
      </Link>
    </section>
  );
}
