import { Navigate } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button, FocusPage, initialsOf, Visor, WorkspaceMark } from "~/lib/ui";
import { useWorkspaces, type Workspace } from "~/modules/workspaces";

import { notionStartError, useStartNotion, type NotionStartError } from "../../api/notion";

/** Navigate the whole page (the consent leaves the app for Notion); tests pass a spy. */
const leavePage = (url: string) => {
  globalThis.location.assign(url);
};

export interface ConnectNotionProps {
  leave?: ((url: string) => void) | undefined;
  /** Back to the app (the start page). */
  onBack: () => void;
}

/**
 * `/connect/notion`: where Notion's listing sends people ("Installation
 * URL"). It only gets to the connect flow, which is the Settings card's:
 * members only, bound to this browser. One workspace starts it at once; several
 * ask which one; an instance without Notion says so.
 */
export function ConnectNotion({ leave = leavePage, onBack }: ConnectNotionProps) {
  const { t } = useTranslation("connectors");
  const workspaces = useWorkspaces();
  const [chosen, setChosen] = useState<string | undefined>();
  if (workspaces.isPending) {
    return (
      <FocusPage
        leading={<Visor state="thinking" size="xl" announce />}
        title={t("connectNotion.opening")}
      />
    );
  }
  if (workspaces.isError) {
    return (
      <FocusPage
        leading={<Visor state="thinking" size="xl" />}
        title={t("connectNotion.loadError.title")}
        lead={t("connectNotion.loadError.lead")}
        actions={
          <Button
            variant="primary"
            size="lg"
            onClick={() => {
              void workspaces.refetch();
            }}
          >
            {t("connectNotion.retry")}
          </Button>
        }
      />
    );
  }
  const list = workspaces.data;
  if (list.length === 0) {
    return <Navigate to="/welcome" replace />;
  }
  const only = list.length === 1 ? list[0] : undefined;
  const target = only ?? list.find((workspace) => workspace.id === chosen);
  if (target !== undefined) {
    return <StartIn key={target.id} workspace={target} leave={leave} onBack={onBack} />;
  }
  return (
    <FocusPage
      leading={<Visor size="xl" />}
      title={t("connectNotion.choose")}
      lead={t("connectNotion.chooseLead")}
    >
      <div className="flex flex-col gap-2" role="group" aria-label={t("connectNotion.choose")}>
        {list.map((workspace) => (
          <Button
            key={workspace.id}
            variant="secondary"
            size="lg"
            leading={<WorkspaceMark mark={initialsOf(workspace.name)} accent={workspace.accent} />}
            onClick={() => {
              setChosen(workspace.id);
            }}
          >
            {workspace.name}
          </Button>
        ))}
      </div>
    </FocusPage>
  );
}

/** Starts the connect flow for `workspace` once, and says what happens if it cannot. */
function StartIn({
  workspace,
  leave,
  onBack,
}: {
  workspace: Workspace;
  leave: (url: string) => void;
  onBack: () => void;
}) {
  const { t } = useTranslation("connectors");
  const start = useStartNotion(workspace.id);
  const [refused, setRefused] = useState<NotionStartError | undefined>();
  const started = useRef(false);
  const begin = () => {
    setRefused(undefined);
    start.mutate(
      { return_to: `/w/${workspace.id}/settings/mailboxes` },
      {
        onSuccess: (answer) => {
          leave(answer.authorize_url);
        },
        onError: (error) => {
          setRefused(notionStartError(error));
        },
      },
    );
  };
  const latest = useRef(begin);
  useEffect(() => {
    latest.current = begin;
  });
  useEffect(() => {
    if (!started.current) {
      started.current = true;
      latest.current();
    }
  }, []);

  if (refused === undefined) {
    return (
      <FocusPage
        leading={<Visor state="thinking" size="xl" announce />}
        title={t("connectNotion.opening")}
        lead={t("connectNotion.openingLead", { workspace: workspace.name })}
      />
    );
  }
  return (
    <FocusPage
      leading={<Visor state="thinking" size="xl" />}
      title={t(`connectNotion.refused.${refused}.title`)}
      lead={t(`connectNotion.refused.${refused}.lead`)}
      actions={
        <>
          {refused === "unavailable" || refused === "not_member" ? null : (
            <Button variant="primary" size="lg" onClick={begin}>
              {t("connectNotion.retry")}
            </Button>
          )}
          <Button variant="secondary" size="lg" onClick={onBack}>
            {t("connectNotion.back")}
          </Button>
        </>
      }
    />
  );
}
