import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Segmented, SettingsPage } from "~/lib/ui";

import { SignupCodes } from "./SignupCodes";
import { SignupRequests } from "./SignupRequests";

type Part = "requests" | "codes";

/**
 * Admin → Signups: who may create an account on this instance. Requests
 * wait for an administrator when signups need approval; codes are the keys
 * administrators hand out when they need a code, each good for the number
 * of signups set when it was minted. The admin
 * layout already turned non-administrators away.
 */
export function SignupsAdminPanel() {
  const { t } = useTranslation("workspaces");
  const [part, setPart] = useState<Part>("requests");

  return (
    <SettingsPage
      label={t("signups.title")}
      title={t("signups.title")}
      lead={t(`signups.lead.${part}`)}
      actions={
        <Segmented
          label={t("signups.parts.label")}
          value={part}
          onChange={setPart}
          options={[
            { value: "requests", label: t("signups.parts.requests") },
            { value: "codes", label: t("signups.parts.codes") },
          ]}
        />
      }
    >
      {part === "requests" ? <SignupRequests /> : <SignupCodes />}
    </SettingsPage>
  );
}
