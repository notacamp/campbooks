import type { ComponentType } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  Fact,
  FactList,
  SettingsPage,
  SettingsSection,
  SkeletonList,
  Text,
} from "~/lib/ui";

import { useProfile } from "../../api/queries";
import { TeachScout } from "../TeachScout/TeachScout";

export interface MemoryPageProps {
  workspaceId: string;
  /** The rules list (the conversations module's), when the app plugs one in. */
  rules?: ComponentType<{ workspaceId: string }> | undefined;
  /** Opens the questions. */
  onQuestions: () => void;
}

/**
 * Settings → Scout's memory: teach Scout in your own words, the rules it
 * runs (the conversations module's list), and your answers to its questions.
 */
export function MemoryPage({ workspaceId, rules: Rules, onQuestions }: MemoryPageProps) {
  const { t } = useTranslation("assistant");
  const profile = useProfile(workspaceId);
  const answered = profile.data?.asked_at !== null && profile.data?.asked_at !== undefined;
  useShortcut("scout.questions", onQuestions);
  return (
    <SettingsPage label={t("memory.title")} title={t("memory.title")} lead={t("memory.lead")}>
      <SettingsSection title={t("memory.teach.title")} description={t("memory.teach.description")}>
        <TeachScout workspaceId={workspaceId} />
      </SettingsSection>
      {Rules === undefined ? null : (
        <SettingsSection title={t("memory.rules")} description={t("memory.rulesDescription")}>
          <Rules workspaceId={workspaceId} />
        </SettingsSection>
      )}
      <SettingsSection
        title={t("memory.questions.title")}
        description={t("memory.questions.description")}
      >
        {profile.isPending ? (
          <SkeletonList rows={1} />
        ) : profile.isError ? (
          // Not "never answered": Scout couldn't tell.
          <Banner
            tone="danger"
            role="alert"
            title={t("memory.questions.failed")}
            action={
              <Button
                variant="secondary"
                onClick={() => {
                  void profile.refetch();
                }}
              >
                {t("memory.teach.tryAgain")}
              </Button>
            }
          />
        ) : answered ? (
          <FactList>
            <Fact label={t("memory.questions.proactivity")}>
              {profile.data.proactivity === "act_then_tell"
                ? t("memory.questions.actThenTell")
                : t("memory.questions.askFirst")}
            </Fact>
          </FactList>
        ) : (
          <Text tone="secondary">{t("memory.questions.never")}</Text>
        )}
        <div>
          <Button shortcut="scout.questions" onClick={onQuestions}>
            {answered ? t("memory.questions.again") : t("memory.questions.start")}
          </Button>
        </div>
      </SettingsSection>
    </SettingsPage>
  );
}
