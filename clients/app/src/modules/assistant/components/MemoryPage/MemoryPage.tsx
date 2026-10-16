import type { ComponentType } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  Heading,
  PaneHeader,
  ReaderBody,
  ReaderPane,
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
    <ReaderPane label={t("memory.title")}>
      <PaneHeader>
        <Heading level={1} size="pane">
          {t("memory.title")}
        </Heading>
      </PaneHeader>
      <ReaderBody>
        <Text tone="secondary">{t("memory.lead")}</Text>
        <section className="flex flex-col gap-3" aria-labelledby="teach-scout">
          <Heading level={2} size="section" id="teach-scout">
            {t("memory.teach.title")}
          </Heading>
          <TeachScout workspaceId={workspaceId} />
        </section>
        {Rules === undefined ? null : (
          <section className="flex flex-col gap-3" aria-labelledby="scout-rules">
            <Heading level={2} size="section" id="scout-rules">
              {t("memory.rules")}
            </Heading>
            <Rules workspaceId={workspaceId} />
          </section>
        )}
        <section className="flex flex-col items-start gap-3" aria-labelledby="scout-answers">
          <Heading level={2} size="section" id="scout-answers">
            {t("memory.questions.title")}
          </Heading>
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
          ) : (
            <Text>
              {answered
                ? profile.data.proactivity === "act_then_tell"
                  ? t("memory.questions.actThenTell")
                  : t("memory.questions.askFirst")
                : t("memory.questions.never")}
            </Text>
          )}
          <Button shortcut="scout.questions" onClick={onQuestions}>
            {answered ? t("memory.questions.again") : t("memory.questions.start")}
          </Button>
        </section>
      </ReaderBody>
    </ReaderPane>
  );
}
