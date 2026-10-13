import { useLocation } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { Brand, ButtonLink, FocusPage, Visor } from "~/lib/ui";

/** The sign-in screen, coming back to `returnTo`. */
function signInPath(returnTo: string): string {
  return `/sign-in?return_to=${encodeURIComponent(returnTo)}`;
}

/**
 * A session ended while the app was open: say so in place (leaving the page
 * could lose what the person was doing), with "Sign in again" back to it.
 */
export function SignedOut() {
  const { t } = useTranslation();
  const { href } = useLocation();
  return (
    <FocusPage
      top={<Brand name={t("appName")} />}
      leading={<Visor size="xl" state="asleep" />}
      title={t("landing.ended")}
      lead={t("landing.endedDetail")}
      footer={t("landing.footer")}
      actions={
        <ButtonLink variant="primary" size="lg" href={signInPath(href)}>
          {t("landing.signInAgain")}
        </ButtonLink>
      }
    />
  );
}
