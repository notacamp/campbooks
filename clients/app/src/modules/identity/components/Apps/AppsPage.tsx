import { useId, useRef, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  Avatar,
  Banner,
  Button,
  ChoiceCards,
  CopyField,
  EmptyState,
  Heading,
  InlineConfirm,
  List,
  ListRow,
  SettingsPage,
  SettingsSection,
  SkeletonList,
  Tag,
  Text,
  TextField,
} from "~/lib/ui";
import { useWorkspaces, type Workspace } from "~/modules/workspaces";

import {
  useConnectedApps,
  useRegisterClient,
  useRemoveClient,
  useRevokeConsent,
  useRotateClientSecret,
  useWorkspaceClients,
  type ConnectedApp,
  type RegisteredClient,
  type WorkspaceClient,
} from "../../api/queries";
import { APP_ACCESS, isReturnAddress, leavePage, stepUpUrl, type AppAccess } from "../../model";

export interface AppsPageProps {
  /** The open workspace: whose own apps are shown. */
  workspaceId: string;
  /** This page's own path, to come back to after a fresh sign-in. */
  returnTo: string;
  /** Leaves the app for a fresh sign-in (tests and stories pass a spy). */
  leave?: ((url: string) => void) | undefined;
}

function useDateFormat() {
  const { i18n } = useTranslation();
  const format = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });
  return (iso: string) => format.format(new Date(iso));
}

/** A scope as a sentence: the translation for its name, else the name. */
function useScopeSentence() {
  const { t } = useTranslation("identity");
  return (name: string) =>
    t(`scopes.${name.replaceAll(":", "_")}` as "scopes.openid", { defaultValue: name });
}

/** Whether `error` asks for a fresh sign-in first. */
const needsStepUp = (error: unknown) => problemCode(error) === "step_up_required";

/**
 * Settings → Apps and integrations: the apps the person let in (Revoke,
 * confirmed in place), and the open workspace's own apps for its owners and
 * admins (New app, Rotate secret, Remove). A secret is shown once, with
 * Copy, and never again.
 */
export function AppsPage({ workspaceId, returnTo, leave = leavePage }: AppsPageProps) {
  const { t } = useTranslation("identity");
  const workspaces = useWorkspaces();
  const workspace = workspaces.data?.find((w) => w.id === workspaceId);
  const admin = workspace?.role === "owner" || workspace?.role === "admin";
  const nameField = useRef<HTMLInputElement>(null);
  const [creating, setCreating] = useState(false);

  const startNew = () => {
    setCreating(true);
    // The form mounts first.
    queueMicrotask(() => nameField.current?.focus());
  };
  useShortcut("identity.register_client", startNew, { enabled: admin });

  return (
    <SettingsPage
      label={t("apps.title")}
      title={t("apps.title")}
      lead={t("apps.lead")}
      actions={
        admin ? (
          <Button icon="plus" shortcut="identity.register_client" onClick={startNew}>
            {t("apps.workspace.new")}
          </Button>
        ) : undefined
      }
    >
      <ConnectedApps workspaces={workspaces.data ?? []} />
      {workspace === undefined ? null : (
        <WorkspaceApps
          workspace={workspace}
          admin={admin}
          creating={creating}
          nameField={nameField}
          onDoneCreating={() => {
            setCreating(false);
          }}
          returnTo={returnTo}
          leave={leave}
        />
      )}
    </SettingsPage>
  );
}

function LoadError({ onRetry }: { onRetry: () => void }) {
  const { t } = useTranslation("identity");
  return (
    <EmptyState
      title={t("apps.loadError.title")}
      visor="thinking"
      visorSize="lg"
      headingLevel={3}
      action={<Button onClick={onRetry}>{t("apps.loadError.retry")}</Button>}
    >
      {t("apps.loadError.lead")}
    </EmptyState>
  );
}

function ConnectedApps({ workspaces }: { workspaces: readonly Workspace[] }) {
  const { t } = useTranslation("identity");
  const apps = useConnectedApps();
  return (
    <SettingsSection
      title={t("apps.connected.title")}
      count={apps.data?.length}
      description={t("apps.connected.lead")}
    >
      {apps.isPending ? (
        <div role="status" aria-label={t("apps.loading")}>
          <SkeletonList rows={2} />
        </div>
      ) : apps.isError ? (
        <LoadError
          onRetry={() => {
            void apps.refetch();
          }}
        />
      ) : apps.data.length === 0 ? (
        <Text tone="tertiary" size="sm">
          {t("apps.connected.empty")}
        </Text>
      ) : (
        <List label={t("apps.connected.list")}>
          {apps.data.map((app) => (
            <ConnectedAppRow key={app.id} app={app} workspaces={workspaces} />
          ))}
        </List>
      )}
    </SettingsSection>
  );
}

function ConnectedAppRow({
  app,
  workspaces,
}: {
  app: ConnectedApp;
  workspaces: readonly Workspace[];
}) {
  const { t } = useTranslation("identity");
  const formatDate = useDateFormat();
  const sentence = useScopeSentence();
  const revoke = useRevokeConsent();
  const [confirming, setConfirming] = useState(false);
  const name = app.client_name ?? t("apps.connected.removedApp");
  const boundName = workspaces.find((w) => w.id === app.workspace_id)?.name;
  const where =
    app.workspace_id === null || app.workspace_id === undefined
      ? t("apps.connected.allWorkspaces")
      : boundName === undefined
        ? t("apps.connected.elsewhere")
        : t("apps.connected.in", { workspace: boundName });
  const meta = [
    where,
    t("apps.connected.granted", { date: formatDate(app.granted_at) }),
    app.last_used_at === null || app.last_used_at === undefined
      ? null
      : t("apps.connected.lastUsed", { date: formatDate(app.last_used_at) }),
  ]
    .filter((part) => part !== null)
    .join(" · ");
  return (
    <ListRow
      leading={<Avatar name={name} />}
      title={name}
      badge={
        app.verified ? (
          <Tag icon="check">{t("consent.campbooks")}</Tag>
        ) : app.client_kind === "workspace" ? (
          <Tag icon="people">{t("apps.connected.workspaceApp")}</Tag>
        ) : (
          <Tag icon="warning">{t("consent.unverified")}</Tag>
        )
      }
      meta={meta}
      trailing={
        <Button
          variant="ghost"
          aria-label={t("apps.connected.revokeLabel", { app: name })}
          aria-expanded={confirming}
          onClick={() => {
            setConfirming(true);
          }}
        >
          {t("apps.connected.revoke")}
        </Button>
      }
    >
      <Text tone="tertiary" size="sm">
        {app.scopes.map(sentence).join(" · ")}
      </Text>
      {confirming ? (
        <InlineConfirm
          confirmLabel={t("apps.connected.revoke")}
          cancelLabel={t("apps.connected.cancel")}
          busy={revoke.isPending}
          onCancel={() => {
            setConfirming(false);
          }}
          onConfirm={() => {
            revoke.mutate(app.id, {
              onError: () => {
                setConfirming(false);
              },
            });
          }}
        >
          {t("apps.connected.confirmRevoke", { app: name })}
        </InlineConfirm>
      ) : null}
      {revoke.isError ? (
        <Banner tone="danger" role="alert" title={t("apps.connected.revokeFailed")} />
      ) : null}
    </ListRow>
  );
}

function WorkspaceApps({
  workspace,
  admin,
  creating,
  nameField,
  onDoneCreating,
  returnTo,
  leave,
}: {
  workspace: Workspace;
  admin: boolean;
  creating: boolean;
  nameField: RefObject<HTMLInputElement | null>;
  onDoneCreating: () => void;
  returnTo: string;
  leave: (url: string) => void;
}) {
  const { t } = useTranslation("identity");
  const [stepUp, setStepUp] = useState(false);
  const [secret, setSecret] = useState<RegisteredClient | undefined>();
  return (
    <SettingsSection
      title={t("apps.workspace.title")}
      description={t("apps.workspace.lead", { workspace: workspace.name })}
    >
      {admin ? (
        <>
          {stepUp ? <StepUp returnTo={returnTo} leave={leave} /> : null}
          {secret === undefined ? null : (
            <SecretShownOnce
              registered={secret}
              onDone={() => {
                setSecret(undefined);
              }}
            />
          )}
          {creating ? (
            <NewAppForm
              workspace={workspace}
              nameField={nameField}
              onCreated={(registered) => {
                setStepUp(false);
                setSecret(registered);
                onDoneCreating();
              }}
              onStepUp={() => {
                setStepUp(true);
              }}
              onCancel={onDoneCreating}
            />
          ) : null}
          <ClientList
            workspace={workspace}
            onRotated={(rotated) => {
              setStepUp(false);
              setSecret(rotated);
            }}
            onStepUp={() => {
              setStepUp(true);
            }}
          />
        </>
      ) : (
        <Text tone="tertiary" size="sm">
          {t("apps.workspace.notAdmin")}
        </Text>
      )}
    </SettingsSection>
  );
}

/** "Sign in again first", with the way to do it. */
function StepUp({ returnTo, leave }: { returnTo: string; leave: (url: string) => void }) {
  const { t } = useTranslation("identity");
  return (
    <Banner
      role="alert"
      icon="key"
      title={t("apps.workspace.stepUp.title")}
      action={
        <Button
          variant="secondary"
          onClick={() => {
            leave(stepUpUrl(returnTo));
          }}
        >
          {t("apps.workspace.stepUp.action")}
        </Button>
      }
    >
      {t("apps.workspace.stepUp.lead")}
    </Banner>
  );
}

/** The client id (and a confidential app's secret), this once. */
function SecretShownOnce({
  registered,
  onDone,
}: {
  registered: RegisteredClient;
  onDone: () => void;
}) {
  const { t } = useTranslation("identity");
  const copy = {
    copyLabel: t("apps.workspace.copy"),
    copiedLabel: t("apps.workspace.copied"),
    failedLabel: t("apps.workspace.copyFailed"),
  };
  const secret = registered.client_secret;
  return (
    <Banner
      role="status"
      icon={secret === null || secret === undefined ? "check" : "key"}
      title={
        secret === null || secret === undefined
          ? registered.client.name
          : t("apps.workspace.secretTitle")
      }
      action={
        <Button variant="secondary" onClick={onDone}>
          {t("apps.workspace.done")}
        </Button>
      }
    >
      <div className="flex flex-col gap-3">
        {secret === null || secret === undefined ? null : (
          <Text size="sm">{t("apps.workspace.secretLead")}</Text>
        )}
        <CopyField
          label={t("apps.workspace.clientId")}
          value={registered.client.client_id}
          {...copy}
        />
        {secret === null || secret === undefined ? null : (
          <CopyField label={t("apps.workspace.secret")} value={secret} {...copy} />
        )}
      </div>
    </Banner>
  );
}

function NewAppForm({
  workspace,
  nameField,
  onCreated,
  onStepUp,
  onCancel,
}: {
  workspace: Workspace;
  nameField: RefObject<HTMLInputElement | null>;
  onCreated: (registered: RegisteredClient) => void;
  onStepUp: () => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation("identity");
  const register = useRegisterClient(workspace.id);
  const titleId = useId();
  const [name, setName] = useState("");
  const [type, setType] = useState<"confidential" | "public">("confidential");
  const [access, setAccess] = useState<AppAccess>("read");
  const [redirect, setRedirect] = useState("");
  const [errors, setErrors] = useState<{ name?: string; redirect?: string; form?: string }>({});

  const submit = () => {
    const trimmed = name.trim();
    const next: typeof errors = {};
    if (trimmed === "") {
      next.name = t("apps.workspace.errors.name");
    }
    if (type === "public" && !isReturnAddress(redirect.trim())) {
      next.redirect = t("apps.workspace.errors.redirect");
    }
    setErrors(next);
    if (next.name !== undefined || next.redirect !== undefined) {
      return;
    }
    register.mutate(
      {
        name: trimmed,
        type,
        scopes: [...APP_ACCESS[access]],
        redirect_uris: type === "public" ? [redirect.trim()] : [],
      },
      {
        onSuccess: onCreated,
        onError: (failure) => {
          if (needsStepUp(failure)) {
            onStepUp();
          } else {
            setErrors({ form: t("apps.workspace.errors.failed") });
          }
        },
      },
    );
  };

  return (
    <form
      className="flex flex-col gap-4"
      aria-labelledby={titleId}
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <Heading level={3} size="section" id={titleId}>
        {t("apps.workspace.new")}
      </Heading>
      <TextField
        ref={nameField}
        label={t("apps.workspace.name")}
        placeholder={t("apps.workspace.namePlaceholder")}
        autoComplete="off"
        value={name}
        error={errors.name}
        onChange={(event) => {
          setName(event.target.value);
        }}
      />
      <ChoiceCards
        label={t("apps.workspace.type")}
        value={type}
        onChange={setType}
        options={(["confidential", "public"] as const).map((value) => ({
          value,
          title: t(`apps.workspace.types.${value}`),
          description: t(`apps.workspace.typeLeads.${value}`),
        }))}
      />
      <ChoiceCards
        label={t("apps.workspace.access")}
        value={access}
        onChange={setAccess}
        options={(["read", "write"] as const).map((value) => ({
          value,
          title: t(`apps.workspace.accessOptions.${value}`),
          description: t(`apps.workspace.accessLeads.${value}`),
        }))}
      />
      {type === "public" ? (
        <TextField
          type="url"
          label={t("apps.workspace.redirect")}
          hint={t("apps.workspace.redirectHint")}
          placeholder="https://"
          autoComplete="off"
          value={redirect}
          error={errors.redirect}
          onChange={(event) => {
            setRedirect(event.target.value);
          }}
        />
      ) : null}
      {errors.form === undefined ? null : <Banner tone="danger" role="alert" title={errors.form} />}
      <div className="flex flex-col gap-2 sm:flex-row">
        <Button type="submit" variant="primary" disabled={register.isPending}>
          {register.isPending ? t("apps.workspace.creating") : t("apps.workspace.create")}
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          {t("apps.workspace.cancel")}
        </Button>
      </div>
    </form>
  );
}

function ClientList({
  workspace,
  onRotated,
  onStepUp,
}: {
  workspace: Workspace;
  onRotated: (rotated: RegisteredClient) => void;
  onStepUp: () => void;
}) {
  const { t } = useTranslation("identity");
  const clients = useWorkspaceClients(workspace.id);
  if (clients.isPending) {
    return (
      <div role="status" aria-label={t("apps.loading")}>
        <SkeletonList rows={1} />
      </div>
    );
  }
  if (clients.isError) {
    return (
      <LoadError
        onRetry={() => {
          void clients.refetch();
        }}
      />
    );
  }
  if (clients.data.length === 0) {
    return (
      <Text tone="tertiary" size="sm">
        {t("apps.workspace.empty")}
      </Text>
    );
  }
  return (
    <List label={t("apps.workspace.list", { workspace: workspace.name })}>
      {clients.data.map((client) => (
        <ClientRow
          key={client.client_id}
          workspace={workspace}
          client={client}
          onRotated={onRotated}
          onStepUp={onStepUp}
        />
      ))}
    </List>
  );
}

function ClientRow({
  workspace,
  client,
  onRotated,
  onStepUp,
}: {
  workspace: Workspace;
  client: WorkspaceClient;
  onRotated: (rotated: RegisteredClient) => void;
  onStepUp: () => void;
}) {
  const { t } = useTranslation("identity");
  const formatDate = useDateFormat();
  const rotate = useRotateClientSecret(workspace.id);
  const remove = useRemoveClient(workspace.id);
  const [confirming, setConfirming] = useState<"rotate" | "remove" | undefined>();
  const [failed, setFailed] = useState(false);
  const meta = [
    client.client_id,
    t("apps.workspace.created", { date: formatDate(client.created_at) }),
    client.last_used_at === null || client.last_used_at === undefined
      ? t("apps.workspace.neverUsed")
      : t("apps.workspace.lastUsed", { date: formatDate(client.last_used_at) }),
  ].join(" · ");
  const fail = (failure: unknown) => {
    setConfirming(undefined);
    if (needsStepUp(failure)) {
      onStepUp();
    } else {
      setFailed(true);
    }
  };
  return (
    <ListRow
      leading={<Avatar name={client.name} kind="service" />}
      title={client.name}
      badge={<Tag>{t(`apps.workspace.types.${client.type}`)}</Tag>}
      meta={meta}
      trailing={
        <>
          {client.type === "confidential" ? (
            <Button
              variant="ghost"
              aria-label={t("apps.workspace.rotateLabel", { app: client.name })}
              aria-expanded={confirming === "rotate"}
              onClick={() => {
                setFailed(false);
                setConfirming("rotate");
              }}
            >
              {t("apps.workspace.rotate")}
            </Button>
          ) : null}
          <Button
            variant="ghost"
            aria-label={t("apps.workspace.removeLabel", { app: client.name })}
            aria-expanded={confirming === "remove"}
            onClick={() => {
              setFailed(false);
              setConfirming("remove");
            }}
          >
            {t("apps.workspace.remove")}
          </Button>
        </>
      }
    >
      {confirming === "rotate" ? (
        <InlineConfirm
          confirmLabel={t("apps.workspace.rotate")}
          cancelLabel={t("apps.workspace.cancel")}
          busy={rotate.isPending}
          onCancel={() => {
            setConfirming(undefined);
          }}
          onConfirm={() => {
            rotate.mutate(client.client_id, {
              onSuccess: (rotated) => {
                setConfirming(undefined);
                onRotated(rotated);
              },
              onError: fail,
            });
          }}
        >
          {t("apps.workspace.confirmRotate", { app: client.name })}
        </InlineConfirm>
      ) : null}
      {confirming === "remove" ? (
        <InlineConfirm
          confirmLabel={t("apps.workspace.remove")}
          cancelLabel={t("apps.workspace.cancel")}
          busy={remove.isPending}
          onCancel={() => {
            setConfirming(undefined);
          }}
          onConfirm={() => {
            remove.mutate(client.client_id, { onError: fail });
          }}
        >
          {t("apps.workspace.confirmRemove", { app: client.name })}
        </InlineConfirm>
      ) : null}
      {failed ? (
        <Banner tone="danger" role="alert" title={t("apps.workspace.errors.failed")} />
      ) : null}
    </ListRow>
  );
}
