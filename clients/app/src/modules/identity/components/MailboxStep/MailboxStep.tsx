import { useEffect, useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Brand,
  Button,
  FocusPage,
  initialsOf,
  Text,
  Visor,
  WorkspaceMark,
  type Accent,
} from "~/lib/ui";

import { useAdoptHeldGrant, useHeldGrants, type HeldGrant } from "../../api/queries";

/** The workspace a mailbox step is for (what the first-run flow just created). */
interface StepWorkspace {
  id: string;
  name: string;
  accent: Accent;
}

export interface MailboxStepProps {
  workspace: StepWorkspace;
  /** On to the workspace's Home (after Connect, or Not now). */
  onContinue: () => void;
  /** The first-run flow's own "is ready" view, shown when no mailbox waits. */
  children: ReactNode;
}

/** The sentence for an adoption refusal (`identity.adopt_held_grant`). */
export function adoptFailure(error: unknown): "gone" | "taken" | "unsupported" | "failed" {
  if (errorStatus(error) === 404) {
    return "gone";
  }
  switch (problemCode(error)) {
    case "account_connected_by_another_member": {
      return "taken";
    }
    case "mailbox_unsupported": {
      return "unsupported";
    }
    default: {
      return "failed";
    }
  }
}

/** The day a held grant stops waiting, in the UI's language ("20 October"). */
export function useWaitsUntil(grant: HeldGrant): string {
  const { i18n } = useTranslation();
  return new Intl.DateTimeFormat(i18n.language, { day: "numeric", month: "long" }).format(
    new Date(grant.expires_at),
  );
}

/**
 * After the first workspace is created: when the mailbox from sign-up is
 * held, "Your Gmail … will be connected to this workspace", Connect (Enter)
 * or Not now (Esc), both on to Home. Never forced: Home keeps offering it
 * until it expires. With nothing held, the flow's own "is ready" view.
 */
export function MailboxStep({ workspace, onContinue, children }: MailboxStepProps) {
  const { t } = useTranslation("identity");
  const { t: tShell } = useTranslation("shell");
  const held = useHeldGrants();
  const [grant] = held.data ?? [];

  if (held.isPending) {
    return (
      <FocusPage
        top={<Brand name={tShell("appName")} />}
        accent={workspace.accent}
        leading={<Visor state="thinking" size="xl" announce />}
        title={t("mailbox.loading")}
      />
    );
  }
  // Nothing held (or the list can't be read): the flow goes on as before;
  // Home offers the mailbox if it turns up.
  if (grant === undefined) {
    return children;
  }
  return <ConnectStep workspace={workspace} grant={grant} onContinue={onContinue} />;
}

function ConnectStep({
  workspace,
  grant,
  onContinue,
}: {
  workspace: StepWorkspace;
  grant: HeldGrant;
  onContinue: () => void;
}) {
  const { t } = useTranslation("identity");
  const { t: tShell } = useTranslation("shell");
  const adopt = useAdoptHeldGrant(workspace.id);
  const connect = useRef<HTMLButtonElement>(null);
  const until = useWaitsUntil(grant);
  useEffect(() => {
    connect.current?.focus();
  }, []);
  useShortcut("overlay.close", onContinue);

  const failure = adopt.isError ? adoptFailure(adopt.error) : undefined;
  const busy = adopt.isPending || adopt.isSuccess;
  // A refusal that retrying won't fix: the only way is on.
  const settled = failure !== undefined && failure !== "failed";

  return (
    <FocusPage
      top={<Brand name={tShell("appName")} />}
      accent={workspace.accent}
      leading={
        <WorkspaceMark mark={initialsOf(workspace.name)} accent={workspace.accent} size="lg" />
      }
      eyebrow={t("mailbox.eyebrow", { workspace: workspace.name })}
      title={
        grant.provider === "google"
          ? t("mailbox.title", { address: grant.address })
          : t("mailbox.titleOther", { address: grant.address })
      }
      lead={
        <>
          <p>{t("mailbox.lead")}</p>
          <p>{t("mailbox.later", { date: until })}</p>
        </>
      }
      formLabel={t("mailbox.formLabel")}
      onSubmit={() => {
        if (settled) {
          onContinue();
          return;
        }
        if (!busy) {
          adopt.mutate(grant.id, { onSuccess: onContinue });
        }
      }}
      actions={
        settled ? (
          <Button ref={connect} type="submit" variant="primary" size="lg" keys="enter">
            {t("mailbox.continue")}
          </Button>
        ) : (
          <>
            <Button
              ref={connect}
              type="submit"
              variant="primary"
              size="lg"
              icon="mail"
              keys="enter"
              disabled={busy}
            >
              {busy ? t("mailbox.connecting") : t("mailbox.connect")}
            </Button>
            <Button
              variant="ghost"
              size="lg"
              shortcut="overlay.close"
              disabled={busy}
              onClick={onContinue}
            >
              {t("mailbox.notNow")}
            </Button>
          </>
        )
      }
    >
      <Text tone="tertiary" size="sm">
        {t("mailbox.signIn")}
      </Text>
      {failure === undefined ? undefined : (
        <Banner tone="danger" role="alert" title={t(`mailbox.errors.${failure}.title`)}>
          {t(`mailbox.errors.${failure}.lead`, { address: grant.address })}
        </Banner>
      )}
    </FocusPage>
  );
}
