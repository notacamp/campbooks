import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { useSession } from "~/lib/api";
import { isTauri } from "~/lib/platform";
import { Brand, Button, FocusPage, Visor } from "~/lib/ui";

import { Landing } from "./Landing";

/**
 * Renders the app only for a signed-in browser. Signed out, it shows the
 * landing at the requested URL (so signing in comes back to it); if a live
 * session ends while the app is open, the landing says so. The native shell
 * owns its own sign-in, so the gate is web only.
 */
export function SessionGate({ children }: { children: ReactNode }) {
  return isTauri() ? children : <WebSessionGate>{children}</WebSessionGate>;
}

function WebSessionGate({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const session = useSession();
  const [wasSignedIn, setWasSignedIn] = useState(false);
  const signedIn = session.data?.authenticated === true;
  if (signedIn && !wasSignedIn) {
    setWasSignedIn(true);
  }

  if (session.isPending) {
    return (
      <FocusPage
        top={<Brand name={t("appName")} />}
        leading={<Visor state="thinking" size="xl" announce />}
        title={t("gate.opening")}
      />
    );
  }
  if (session.isError && session.data === undefined) {
    return (
      <FocusPage
        top={<Brand name={t("appName")} />}
        leading={<Visor state="thinking" size="xl" />}
        title={t("gate.unreachableTitle")}
        lead={t("gate.unreachableLead")}
        actions={
          <Button
            variant="primary"
            size="lg"
            onClick={() => {
              void session.refetch();
            }}
          >
            {t("gate.retry")}
          </Button>
        }
      />
    );
  }
  if (!signedIn) {
    return <Landing ended={wasSignedIn} />;
  }
  return children;
}
