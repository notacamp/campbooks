import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

export function NotFound() {
  const { t } = useTranslation();
  return (
    <section className="space-y-2">
      <h1 className="text-xl font-semibold">{t("notFound.title")}</h1>
      <p>{t("notFound.body")}</p>
      <Link to="/" className="underline">
        {t("notFound.home")}
      </Link>
    </section>
  );
}
