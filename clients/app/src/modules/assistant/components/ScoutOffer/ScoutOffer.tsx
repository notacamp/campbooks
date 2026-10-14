import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { Button, Note } from "~/lib/ui";

import { useProfile, useUpdateProfile } from "../../api/queries";

export interface ScoutOfferProps {
  workspaceId: string;
}

/**
 * Home's offer of the questions, once mail is connected: Start, or Not now
 * (it won't ask again; Scout's memory keeps the way back). Nothing while the
 * profile loads or once the questions were answered or put off.
 */
export function ScoutOffer({ workspaceId }: ScoutOfferProps) {
  const { t } = useTranslation("assistant");
  const navigate = useNavigate();
  const profile = useProfile(workspaceId);
  const update = useUpdateProfile(workspaceId);
  if (profile.data === undefined || (profile.data.asked_at ?? null) !== null) {
    return null;
  }
  const current = profile.data;
  return (
    <Note
      visor="happy"
      label={t("offer.title")}
      actions={
        <>
          <Button
            variant="primary"
            onClick={() => {
              void navigate({ to: `/scout/${workspaceId}` });
            }}
          >
            {t("offer.start")}
          </Button>
          <Button
            variant="ghost"
            disabled={update.isPending}
            onClick={() => {
              update.mutate({ proactivity: current.proactivity, asked: true });
            }}
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
