import { useId, useRef, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode, useSession } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  Avatar,
  Banner,
  Button,
  EmptyState,
  Heading,
  InlineConfirm,
  List,
  ListRow,
  PaneHeader,
  ReaderBody,
  ReaderPane,
  Select,
  SkeletonList,
  Spacer,
  Text,
  TextField,
  useUndoToast,
} from "~/lib/ui";

import {
  useChangeRole,
  useInvitations,
  useInviteMember,
  useMembers,
  useRemoveMember,
  type Invitation,
  type Member,
  type Role,
  type Workspace,
} from "../../api/queries";
import { assignableRoles, canManage, canRemove, looksLikeEmail } from "../../model";

function useDateFormat() {
  const { i18n } = useTranslation();
  const format = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });
  return (iso: string) => format.format(new Date(iso));
}

function useRoleLabel() {
  const { t } = useTranslation("workspaces");
  return (role: Role) => t(`role.${role}`);
}

/** Why changing or removing a member failed, as a message key. */
function rowErrorKey(error: unknown) {
  if (problemCode(error) === "last_owner") {
    return "members.errors.lastOwner" as const;
  }
  return errorStatus(error) === 403
    ? ("members.errors.forbidden" as const)
    : ("members.errors.generic" as const);
}

/** Settings → Members: who is in the workspace, their roles, and invitations. */
export function MembersPage({ workspace }: { workspace: Workspace }) {
  const { t } = useTranslation("workspaces");
  const manage = canManage(workspace);
  const members = useMembers(workspace.id);
  const emailField = useRef<HTMLInputElement>(null);

  useShortcut(
    "workspaces.invite_member",
    () => {
      emailField.current?.focus();
    },
    { enabled: manage },
  );

  return (
    <ReaderPane label={t("members.title")}>
      <PaneHeader>
        <Heading level={1} size="pane">
          {t("members.title")}
        </Heading>
        <Spacer />
        {manage ? (
          <Button
            icon="plus"
            shortcut="workspaces.invite_member"
            onClick={() => {
              emailField.current?.focus();
            }}
          >
            {t("invite.title")}
          </Button>
        ) : null}
      </PaneHeader>
      <ReaderBody>
        <Text tone="secondary">{t("members.lead", { workspace: workspace.name })}</Text>
        {members.isPending ? (
          <div role="status" aria-label={t("members.loading")}>
            <SkeletonList rows={3} />
          </div>
        ) : members.isError ? (
          <EmptyState
            title={t("members.loadError.title")}
            visor="thinking"
            visorSize="lg"
            action={
              <Button
                onClick={() => {
                  void members.refetch();
                }}
              >
                {t("members.loadError.retry")}
              </Button>
            }
          >
            {t("members.loadError.lead")}
          </EmptyState>
        ) : (
          <MemberList workspace={workspace} members={members.data} />
        )}
        {manage ? (
          <>
            <InviteForm workspace={workspace} emailField={emailField} />
            <PendingInvitations workspace={workspace} />
          </>
        ) : (
          <Text tone="tertiary" size="sm">
            {t("members.readOnly")}
          </Text>
        )}
      </ReaderBody>
    </ReaderPane>
  );
}

function MemberList({ workspace, members }: { workspace: Workspace; members: Member[] }) {
  const { t } = useTranslation("workspaces");
  const session = useSession();
  const myEmail = session.data?.user?.email?.toLowerCase();
  return (
    <List label={t("members.list", { workspace: workspace.name })}>
      {members.map((member) => (
        <MemberRow
          key={member.user_id}
          workspace={workspace}
          member={member}
          isMe={myEmail !== undefined && member.email?.toLowerCase() === myEmail}
        />
      ))}
    </List>
  );
}

function MemberRow({
  workspace,
  member,
  isMe,
}: {
  workspace: Workspace;
  member: Member;
  isMe: boolean;
}) {
  const { t } = useTranslation("workspaces");
  const formatDate = useDateFormat();
  const roleLabel = useRoleLabel();
  const undo = useUndoToast();
  const changeRole = useChangeRole(workspace.id);
  const remove = useRemoveMember(workspace.id);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const name = member.display_name ?? member.email ?? member.user_id;
  const roles = isMe ? [] : assignableRoles(workspace.role, member.role);
  const removable = !isMe && canRemove(workspace.role, member.role);

  const setRole = (role: Role, previous: Role, record: boolean) => {
    setError(undefined);
    changeRole.mutate(
      { userId: member.user_id, role },
      {
        onSuccess: () => {
          if (record) {
            undo.push({
              label: t("members.roleChanged", { name, role: roleLabel(role) }),
              undo: () => {
                setRole(previous, role, false);
              },
              redo: () => {
                setRole(role, previous, false);
              },
            });
          }
        },
        onError: (failure) => {
          setError(t(rowErrorKey(failure)));
        },
      },
    );
  };

  const meta = [
    member.email === name ? null : member.email,
    t("members.joined", { date: formatDate(member.joined_at) }),
  ]
    .filter((part) => part !== null && part !== undefined)
    .join(" · ");

  return (
    <ListRow
      leading={<Avatar name={name} />}
      title={name}
      badge={isMe ? t("members.you") : undefined}
      meta={meta}
      trailing={
        <>
          {roles.length > 0 ? (
            <Select
              hideLabel
              label={t("members.roleOf", { name })}
              value={member.role}
              disabled={changeRole.isPending}
              options={roles.map((role) => ({ value: role, label: roleLabel(role) }))}
              onChange={(role) => {
                setRole(role, member.role, true);
              }}
            />
          ) : (
            <Text as="span" tone="secondary" size="sm">
              {roleLabel(member.role)}
            </Text>
          )}
          {removable ? (
            <Button
              variant="ghost"
              aria-label={t("members.removeLabel", { name })}
              aria-expanded={confirming}
              onClick={() => {
                setError(undefined);
                setConfirming(true);
              }}
            >
              {t("members.remove")}
            </Button>
          ) : null}
        </>
      }
    >
      {confirming ? (
        <InlineConfirm
          confirmLabel={t("members.remove")}
          cancelLabel={t("members.cancel")}
          busy={remove.isPending}
          onCancel={() => {
            setConfirming(false);
          }}
          onConfirm={() => {
            remove.mutate(member.user_id, {
              onError: (failure) => {
                setConfirming(false);
                setError(t(rowErrorKey(failure)));
              },
            });
          }}
        >
          {t("members.confirmRemove", { name, workspace: workspace.name })}
        </InlineConfirm>
      ) : null}
      {error === undefined ? null : <Banner tone="danger" role="alert" title={error} />}
    </ListRow>
  );
}

function InviteForm({
  workspace,
  emailField,
}: {
  workspace: Workspace;
  emailField: RefObject<HTMLInputElement | null>;
}) {
  const { t } = useTranslation("workspaces");
  const roleLabel = useRoleLabel();
  const invite = useInviteMember(workspace.id);
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<"member" | "admin">("member");
  const [error, setError] = useState<string | undefined>();
  const [sent, setSent] = useState<string | undefined>();
  const titleId = useId();

  const submit = () => {
    const address = email.trim();
    setSent(undefined);
    if (!looksLikeEmail(address)) {
      setError(t("invite.errors.email"));
      emailField.current?.focus();
      return;
    }
    setError(undefined);
    invite.mutate(
      { email: address, role },
      {
        onSuccess: (invitation) => {
          setEmail("");
          setSent(t("invite.sent", { email: invitation.email, role: roleLabel(invitation.role) }));
        },
        onError: (failure) => {
          setError(
            problemCode(failure) === "already_a_member"
              ? t("invite.errors.alreadyMember", { email: address })
              : errorStatus(failure) === 400
                ? t("invite.errors.email")
                : t("invite.errors.generic"),
          );
          emailField.current?.focus();
        },
      },
    );
  };

  return (
    <section className="flex flex-col gap-3" aria-labelledby={titleId}>
      <Heading level={2} size="section" id={titleId}>
        {t("invite.title")}
      </Heading>
      <Text tone="tertiary" size="sm">
        {t("invite.lead")}
      </Text>
      <form
        className="flex flex-col gap-3 sm:flex-row sm:items-start"
        aria-labelledby={titleId}
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <div className="flex-1">
          <TextField
            ref={emailField}
            type="email"
            autoComplete="off"
            label={t("invite.email")}
            placeholder={t("invite.placeholder")}
            value={email}
            error={error}
            onChange={(event) => {
              setEmail(event.target.value);
            }}
          />
        </div>
        <Select
          label={t("invite.role")}
          value={role}
          onChange={setRole}
          options={[
            { value: "member", label: roleLabel("member") },
            { value: "admin", label: roleLabel("admin") },
          ]}
        />
        <div className="flex sm:pt-6">
          <Button type="submit" variant="primary" disabled={invite.isPending}>
            {invite.isPending ? t("invite.sending") : t("invite.submit")}
          </Button>
        </div>
      </form>
      {sent === undefined ? null : <Banner icon="check" role="status" title={sent} />}
    </section>
  );
}

function PendingInvitations({ workspace }: { workspace: Workspace }) {
  const { t } = useTranslation("workspaces");
  const invitations = useInvitations(workspace.id, true);
  const titleId = useId();
  return (
    <section className="flex flex-col gap-3" aria-labelledby={titleId}>
      <Heading level={2} size="section" id={titleId}>
        {t("invite.pending")}
      </Heading>
      {invitations.isPending ? (
        <SkeletonList rows={1} />
      ) : invitations.isError ? (
        <Banner
          tone="danger"
          title={t("invite.loadError")}
          action={
            <Button
              onClick={() => {
                void invitations.refetch();
              }}
            >
              {t("members.loadError.retry")}
            </Button>
          }
        />
      ) : invitations.data.length === 0 ? (
        <Text tone="tertiary" size="sm">
          {t("invite.none")}
        </Text>
      ) : (
        <List label={t("invite.pending")}>
          {invitations.data.map((invitation) => (
            <InvitationRow key={invitation.id} workspace={workspace} invitation={invitation} />
          ))}
        </List>
      )}
    </section>
  );
}

function InvitationRow({
  workspace,
  invitation,
}: {
  workspace: Workspace;
  invitation: Invitation;
}) {
  const { t } = useTranslation("workspaces");
  const formatDate = useDateFormat();
  const roleLabel = useRoleLabel();
  const invite = useInviteMember(workspace.id);
  const [now] = useState(() => Date.now());
  const expired = new Date(invitation.expires_at).getTime() <= now;
  const details = { role: roleLabel(invitation.role), date: formatDate(invitation.expires_at) };
  return (
    <ListRow
      muted={expired}
      title={invitation.email}
      meta={expired ? t("invite.expired", details) : t("invite.expires", details)}
      trailing={
        expired ? (
          <Button
            variant="ghost"
            aria-label={t("invite.resendLabel", { email: invitation.email })}
            disabled={invite.isPending}
            onClick={() => {
              invite.mutate({ email: invitation.email, role: invitation.role });
            }}
          >
            {t("invite.resend")}
          </Button>
        ) : undefined
      }
    >
      {invite.isError ? (
        <Banner tone="danger" role="alert" title={t("invite.errors.generic")} />
      ) : null}
    </ListRow>
  );
}
