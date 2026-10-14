import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Heading, PaneHeader, ReaderBody, ReaderPane, Segmented, Spacer, Text } from "~/lib/ui";

import { SignupCodes } from "./SignupCodes";
import { SignupRequests } from "./SignupRequests";

type Part = "requests" | "codes";

/**
 * Admin → Signups: who may create an account on this instance. Requests
 * wait for an administrator when signups need approval; codes are the
 * single-use keys administrators hand out when they need a code. The admin
 * layout already turned non-administrators away.
 */
export function SignupsAdminPanel() {
  const { t } = useTranslation("workspaces");
  const [part, setPart] = useState<Part>("requests");

  return (
    <ReaderPane label={t("signups.title")}>
      <PaneHeader>
        <Heading level={1} size="pane">
          {t("signups.title")}
        </Heading>
        <Spacer />
        <Segmented
          label={t("signups.parts.label")}
          value={part}
          onChange={setPart}
          options={[
            { value: "requests", label: t("signups.parts.requests") },
            { value: "codes", label: t("signups.parts.codes") },
          ]}
        />
      </PaneHeader>
      <ReaderBody>
        <Text tone="secondary">{t(`signups.lead.${part}`)}</Text>
        {part === "requests" ? <SignupRequests /> : <SignupCodes />}
      </ReaderBody>
    </ReaderPane>
  );
}
