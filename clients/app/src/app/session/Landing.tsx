import { useTranslation } from "react-i18next";

import { loginUrl } from "~/lib/api";
import { Banner, Brand, ButtonLink, FocusPage, Visor } from "~/lib/ui";

/** Where the browser is now: sign-in brings it back exactly here. */
function here(): string {
  const { pathname, search, hash } = globalThis.location;
  return pathname + search + hash;
}

export interface LandingProps {
  /** A session that was live has ended. */
  ended?: boolean | undefined;
}

/** Signed out: the brand, one line, and "Sign in" (via the BFF, back to this page). */
export function Landing({ ended = false }: LandingProps) {
  const { t } = useTranslation();
  const invited = globalThis.location.pathname.startsWith("/invitations/accept");
  return (
    <FocusPage
      top={<Brand name={t("appName")} />}
      leading={<Visor size="xl" />}
      title={invited ? t("landing.invitedTitle") : t("landing.title")}
      lead={invited ? t("landing.invitedLead") : t("landing.lead")}
      footer={t("landing.footer")}
      actions={
        <ButtonLink variant="primary" size="lg" href={loginUrl(here())}>
          {t("landing.signIn")}
        </ButtonLink>
      }
    >
      {ended ? (
        <Banner role="status" title={t("landing.ended")}>
          {t("landing.endedDetail")}
        </Banner>
      ) : undefined}
    </FocusPage>
  );
}
