import { Navigate, useRouterState } from "@tanstack/react-router";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { useSession } from "~/lib/api";
import { isTauri } from "~/lib/platform";
import { Brand, Button, FocusPage, Visor } from "~/lib/ui";

import { SignedOut } from "./SignedOut";

/**
 * Renders the app only for a signed-in browser. Signed out on load, it goes
 * to the sign-in screen, which comes back to the requested URL; if a live
 * session ends while the app is open, it says so in place. The native shell
 * owns its own sign-in, so the gate is web only.
 */
export function SessionGate({ children }: { children: ReactNode }) {
  return isTauri() ? children : <WebSessionGate>{children}</WebSessionGate>;
}

function WebSessionGate({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const session = useSession();
  // The page this gate guards: while a navigation is pending, the router's
  // latest location is already the next page, not this one.
  const href = useRouterState({
    select: (state) => (state.resolvedLocation ?? state.location).href,
  });
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
    return wasSignedIn ? (
      <SignedOut />
    ) : (
      <Navigate to="/sign-in" search={{ return_to: href }} replace />
    );
  }
  return children;
}
