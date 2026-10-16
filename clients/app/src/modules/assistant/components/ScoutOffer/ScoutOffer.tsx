import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Button, Note, useUndoToast } from "~/lib/ui";

import { useProfile, useUpdateProfile } from "../../api/queries";

export interface ScoutOfferProps {
  workspaceId: string;
}

/**
 * Home's offer of the questions, once mail is connected: Start (T), or Not
 * now (Shift+T: it won't ask again; Scout's memory keeps the way back).
 * Nothing while the profile loads or once the questions were answered or
 * put off.
 */
export function ScoutOffer({ workspaceId }: ScoutOfferProps) {
  const { t } = useTranslation("assistant");
  const navigate = useNavigate();
  const profile = useProfile(workspaceId);
  const update = useUpdateProfile(workspaceId);
  const undo = useUndoToast();
  const shown = profile.data !== undefined && (profile.data.asked_at ?? null) === null;
  const start = () => {
    void navigate({ to: `/scout/${workspaceId}` });
  };
  const notNow = () => {
    if (profile.data === undefined || update.isPending) {
      return;
    }
    update.mutate(
      { proactivity: profile.data.proactivity, asked: true },
      {
        onError: () => {
          undo.notify(t("offer.notNowFailed"));
        },
      },
    );
  };
  useShortcut("scout.offer.start", start, { enabled: shown });
  useShortcut("scout.offer.not_now", notNow, { enabled: shown });
  if (!shown) {
    return null;
  }
  return (
    <Note
      visor="happy"
      label={t("offer.title")}
      actions={
        <>
          <Button variant="primary" shortcut="scout.offer.start" onClick={start}>
            {t("offer.start")}
          </Button>
          <Button
            variant="ghost"
            shortcut="scout.offer.not_now"
            disabled={update.isPending}
            onClick={notNow}
          >
            {t("offer.notNow")}
          </Button>
        </>
      }
    >
      <strong>{t("offer.title")}</strong> {t("offer.lead")}
    </Note>
  );
}
