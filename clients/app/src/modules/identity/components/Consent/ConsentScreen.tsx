import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  Avatar,
  Banner,
  Brand,
  Button,
  ButtonLink,
  ChoiceCards,
  FocusPage,
  Icon,
  List,
  ListRow,
  Tag,
  Text,
  Visor,
  WorkspaceMark,
  initialsOf,
} from "~/lib/ui";

import { useAnswerConsent, useConsentRequest, type ConsentRequest } from "../../api/queries";
import { leavePage } from "../../model";

export interface ConsentScreenProps {
  /** The waiting request the issuer sent the browser here with. */
  request: string;
  /** Full-page navigation (tests and stories pass a spy). */
  leave?: ((url: string) => void) | undefined;
}

/**
 * `/consent`: an app other than Campbooks on the web asks to use the
 * person's account. Shows the app (and whether anyone vouches for it), what
 * it will be able to do, which workspace it may use, and where the browser
 * goes next; Allow (⌘↵) or Deny (Esc). A native app or an agent on this
 * device comes with a warning, and is asked every time.
 */
export function ConsentScreen({ request, leave = leavePage }: ConsentScreenProps) {
  const { t } = useTranslation("identity");
  const asked = useConsentRequest(request);

  if (asked.isPending) {
    return <Simple visor="thinking" title={t("consent.loading")} announce />;
  }
  if (asked.isError) {
    if (errorStatus(asked.error) === 404) {
      return (
        <Simple
          visor="unsure"
          title={t("consent.gone.title")}
          lead={t("consent.gone.lead")}
          action={
            <ButtonLink variant="secondary" size="lg" href="/">
              {t("consent.home")}
            </ButtonLink>
          }
        />
      );
    }
    return (
      <Simple
        visor="unsure"
        title={t("consent.failed.title")}
        lead={t("consent.failed.lead")}
        action={
          <Button
            variant="primary"
            size="lg"
            onClick={() => {
              void asked.refetch();
            }}
          >
            {t("consent.retry")}
          </Button>
        }
      />
    );
  }
  return <Ask request={request} asked={asked.data} leave={leave} />;
}

function Simple({
  visor,
  title,
  lead,
  action,
  announce = false,
}: {
  visor: "thinking" | "unsure";
  title: string;
  lead?: string | undefined;
  action?: ReactNode;
  announce?: boolean;
}) {
  const { t } = useTranslation("shell");
  return (
    <FocusPage
      top={<Brand name={t("appName")} />}
      leading={<Visor state={visor} size="xl" announce={announce} />}
      title={title}
      lead={lead}
      actions={action}
    />
  );
}

/** A scope as a sentence: the translation for its name, else the server's. */
function useScopeSentence() {
  const { t } = useTranslation("identity");
  return (name: string, description: string) =>
    t(`scopes.${name.replaceAll(":", "_")}` as "scopes.openid", {
      defaultValue: description === "" ? name : description.replace(/\.$/u, ""),
    });
}

function Ask({
  request,
  asked,
  leave,
}: {
  request: string;
  asked: ConsentRequest;
  leave: (url: string) => void;
}) {
  const { t } = useTranslation("identity");
  const { t: tShell } = useTranslation("shell");
  const sentence = useScopeSentence();
  const answer = useAnswerConsent(request);
  const allowButton = useRef<HTMLButtonElement>(null);
  const canId = useId();
  const { client } = asked;
  const choices = asked.workspaces;
  const [workspace, setWorkspace] = useState<string | null>(
    choices.length === 1 ? (choices[0]?.id ?? null) : null,
  );
  const busy = answer.isPending || answer.isSuccess;
  const noWorkspace = asked.binds_workspace && choices.length === 0;
  const canAllow = !busy && !noWorkspace && (!asked.binds_workspace || workspace !== null);
  const code = answer.isError ? problemCode(answer.error) : undefined;
  const gone = answer.isError && errorStatus(answer.error) === 404;

  useEffect(() => {
    allowButton.current?.focus();
  }, []);

  const send = (decision: "allow" | "deny") => {
    if (busy || (decision === "allow" && !canAllow)) {
      return;
    }
    answer.mutate(
      decision === "allow" && asked.binds_workspace
        ? { decision, workspace_id: workspace }
        : { decision },
      {
        onSuccess: (redirect) => {
          leave(redirect);
        },
      },
    );
  };
  useShortcut("identity.answer_consent", () => {
    send("allow");
  });
  useShortcut("overlay.close", () => {
    send("deny");
  });

  if (gone) {
    return <Simple visor="unsure" title={t("consent.gone.title")} lead={t("consent.gone.lead")} />;
  }

  const kind = client.kind as "first_party" | "workspace" | "dynamic" | "metadata_document";
  const host = client.document_host ?? client.redirect_host ?? "";
  return (
    <FocusPage
      top={<Brand name={tShell("appName")} />}
      leading={<Visor size="xl" state={client.verified ? "happy" : "alert"} />}
      eyebrow={t("consent.eyebrow")}
      title={t("consent.title", { app: client.name })}
      lead={
        asked.account_email === null || asked.account_email === undefined
          ? t("consent.leadNoEmail")
          : t("consent.lead", { email: asked.account_email })
      }
      footer={asked.remembered ? t("consent.remembered") : t("consent.everyTime")}
      formLabel={t("consent.form", { app: client.name })}
      onSubmit={() => {
        send("allow");
      }}
      actions={
        <>
          <Button
            ref={allowButton}
            type="submit"
            variant="primary"
            size="lg"
            shortcut="identity.answer_consent"
            disabled={!canAllow}
          >
            {answer.isPending ? t("consent.allowing") : t("consent.allow")}
          </Button>
          <Button
            variant="ghost"
            size="lg"
            shortcut="overlay.close"
            disabled={busy}
            onClick={() => {
              send("deny");
            }}
          >
            {t("consent.deny")}
          </Button>
        </>
      }
    >
      <List label={t("consent.app")}>
        <ListRow
          leading={<Avatar name={client.name} size="lg" />}
          title={client.name}
          badge={
            <Tag icon={client.verified ? "check" : "warning"}>
              {client.verified ? t("consent.verified") : t("consent.unverified")}
            </Tag>
          }
        >
          {/* Wraps, unlike a row's meta: who vouches for the app matters here. */}
          <Text tone="secondary" size="sm">
            {t(`consent.kind.${kind}`, { host })}
          </Text>
        </ListRow>
      </List>
      {client.device ? (
        <Banner icon="warning" title={t("consent.device.title")}>
          {t("consent.device.lead")}
        </Banner>
      ) : client.verified ? null : (
        <Banner icon="warning" title={t("consent.unverifiedNote.title")}>
          {t("consent.unverifiedNote.lead")}
        </Banner>
      )}
      <section className="flex flex-col gap-2" aria-labelledby={canId}>
        <Text weight="semibold" id={canId}>
          {t("consent.can")}
        </Text>
        <ul className="flex flex-col gap-2">
          {asked.scopes.map((scope) => (
            <li key={scope.name} className="flex items-start gap-2">
              <Icon name="check" />
              <Text as="span">{sentence(scope.name, scope.description)}</Text>
            </li>
          ))}
        </ul>
      </section>
      {asked.binds_workspace ? (
        noWorkspace ? (
          <Banner tone="danger" role="alert" title={t("consent.noWorkspace.title")}>
            {t("consent.noWorkspace.lead")}
          </Banner>
        ) : (
          <>
            <ChoiceCards
              label={t("consent.workspace")}
              value={workspace}
              onChange={setWorkspace}
              options={choices.map((choice) => ({
                value: choice.id,
                title: choice.name,
                description: t(`consent.role.${choice.role}`),
                leading: <WorkspaceMark mark={initialsOf(choice.name)} />,
              }))}
            />
            <Text tone="tertiary" size="sm">
              {t("consent.workspaceLead")}
            </Text>
          </>
        )
      ) : (
        <Text tone="secondary">{t("consent.allWorkspaces")}</Text>
      )}
      <Text tone="tertiary" size="sm">
        {client.device || client.redirect_host === null || client.redirect_host === undefined
          ? t("consent.goesToDevice")
          : t("consent.goesTo", { host: client.redirect_host })}
      </Text>
      {code === "step_up_required" ? (
        <Banner
          role="alert"
          icon="key"
          title={t("consent.stepUp.title")}
          action={
            <Button
              variant="secondary"
              onClick={() => {
                leave(`/sign-in?request=${encodeURIComponent(request)}`);
              }}
            >
              {t("consent.stepUp.action")}
            </Button>
          }
        >
          {t("consent.stepUp.lead")}
        </Banner>
      ) : answer.isError ? (
        <Banner tone="danger" role="alert" title={t("consent.answerFailed.title")}>
          {t("consent.answerFailed.lead")}
        </Banner>
      ) : null}
    </FocusPage>
  );
}
