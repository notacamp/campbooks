import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode, useSession } from "~/lib/api";
import { Brand, Button, FocusPage, initialsOf, Visor, WorkspaceMark } from "~/lib/ui";

import { useAcceptInvitation, type Workspace } from "../../api/queries";

export interface AcceptInvitationProps {
  /** The token from the invitation link; undefined when the link has none. */
  token: string | undefined;
  onOpen: (workspace: Workspace) => void;
  /** Go to the user's workspaces (whichever opens first). */
  onHome: () => void;
  onSignOut: () => void;
}

type Outcome = "used" | "expired" | "alreadyMember" | "mismatch" | "notFound" | "failed";

function outcomeOf(error: unknown): Outcome {
  switch (problemCode(error)) {
    case "invitation_used": {
      return "used";
    }
    case "invitation_expired": {
      return "expired";
    }
    case "already_a_member": {
      return "alreadyMember";
    }
    case "invitation_email_mismatch": {
      return "mismatch";
    }
    default: {
      return errorStatus(error) === 404 || errorStatus(error) === 400 ? "notFound" : "failed";
    }
  }
}

/**
 * The page an invitation link opens (signed in: the session gate has already
 * sent a signed-out visitor to sign in and back). Accepts once on arrival.
 */
export function AcceptInvitation({ token, onOpen, onHome, onSignOut }: AcceptInvitationProps) {
  const { t } = useTranslation("workspaces");
  const { t: tShell } = useTranslation("shell");
  const accept = useAcceptInvitation();
  const session = useSession();
  const started = useRef(false);
  const primary = useRef<HTMLButtonElement>(null);
  const { mutate } = accept;

  // Accept exactly once, even when effects run twice (StrictMode).
  useEffect(() => {
    if (token === undefined || token === "" || started.current) {
      return;
    }
    started.current = true;
    mutate(token);
  }, [token, mutate]);

  const settled = accept.isSuccess || accept.isError || token === undefined || token === "";
  useEffect(() => {
    if (settled) {
      primary.current?.focus();
    }
  }, [settled]);

  const top = <Brand name={tShell("appName")} />;

  if (accept.isSuccess) {
    const workspace = accept.data;
    return (
      <FocusPage
        top={top}
        accent={workspace.accent}
        leading={
          <WorkspaceMark mark={initialsOf(workspace.name)} accent={workspace.accent} size="lg" />
        }
        title={t("accept.joined.title", { name: workspace.name })}
        lead={
          workspace.role === null || workspace.role === undefined
            ? undefined
            : t("accept.joined.lead", { role: t(`role.${workspace.role}`) })
        }
        actions={
          <Button
            ref={primary}
            variant="primary"
            size="lg"
            onClick={() => {
              onOpen(workspace);
            }}
          >
            {t("accept.joined.open", { name: workspace.name })}
          </Button>
        }
      />
    );
  }

  if (!settled || accept.isPending) {
    return (
      <FocusPage
        top={top}
        leading={<Visor state="thinking" size="xl" announce />}
        title={t("accept.joining")}
      />
    );
  }

  const outcome: Outcome = accept.isError ? outcomeOf(accept.error) : "notFound";
  const email = session.data?.user?.email ?? "";
  return (
    <FocusPage
      top={top}
      leading={<Visor state={outcome === "failed" ? "thinking" : "unsure"} size="xl" />}
      title={t(`accept.${outcome}.title`)}
      lead={t(`accept.${outcome}.lead`, { email })}
      actions={
        outcome === "mismatch" ? (
          <Button ref={primary} variant="primary" size="lg" icon="sign-out" onClick={onSignOut}>
            {t("accept.signOut")}
          </Button>
        ) : outcome === "failed" && token !== undefined ? (
          <>
            <Button
              ref={primary}
              variant="primary"
              size="lg"
              onClick={() => {
                mutate(token);
              }}
            >
              {t("accept.retry")}
            </Button>
            <Button variant="ghost" size="lg" onClick={onHome}>
              {t("accept.home")}
            </Button>
          </>
        ) : (
          <Button ref={primary} variant="secondary" size="lg" onClick={onHome}>
            {t("accept.home")}
          </Button>
        )
      }
    />
  );
}
