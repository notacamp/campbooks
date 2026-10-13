import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ComponentType,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import {
  Banner,
  Brand,
  Button,
  ChoiceCards,
  FocusPage,
  initialsOf,
  TextField,
  Visor,
  WorkspaceMark,
} from "~/lib/ui";

import { useCreateWorkspace, type Kind, type Workspace } from "../../api/queries";
import { ACCENT_FOR_KIND, MAX_NAME_LENGTH } from "../../model";

type Step = "name" | "kind" | "code" | "inviteOnly" | "approval" | "done";

/** What a step after creating the workspace gets (see `afterCreate`). */
export interface AfterCreateProps {
  /** The workspace just created. */
  workspace: Workspace;
  /** Go on to the workspace's Home. */
  onContinue: () => void;
  /** The flow's own "is ready" view, for when there is nothing to add. */
  children: ReactNode;
}

export interface CreateWorkspaceProps {
  /** The user has no workspace yet: no way out but signing out. */
  firstRun: boolean;
  onOpen: (workspace: Workspace) => void;
  onInvite: (workspace: Workspace) => void;
  /** Leave without creating one (only when it isn't the first). */
  onCancel?: (() => void) | undefined;
  onSignOut: () => void;
  /**
   * A step another module adds once the workspace exists (the mailbox held
   * since sign-up). It renders `children` when it has nothing to ask.
   */
  afterCreate?: ComponentType<AfterCreateProps> | undefined;
}

/**
 * Creating a workspace, one question per view: its name, then what it is for
 * (which picks its accent), then done. Name and kind can be skipped; a
 * signup policy refusal gets a view of its own.
 */
export function CreateWorkspace({
  firstRun,
  onOpen,
  onInvite,
  onCancel,
  onSignOut,
  afterCreate: AfterCreate,
}: CreateWorkspaceProps) {
  const { t } = useTranslation("workspaces");
  const { t: tShell } = useTranslation("shell");
  const create = useCreateWorkspace();
  const [step, setStep] = useState<Step>("name");
  const [name, setName] = useState("");
  const [nameError, setNameError] = useState<string | undefined>();
  const [kind, setKind] = useState<Kind | null>(null);
  const [code, setCode] = useState("");
  const [codeError, setCodeError] = useState<string | undefined>();
  const [failed, setFailed] = useState(false);
  const [created, setCreated] = useState<Workspace | null>(null);
  const focusTarget = useRef<HTMLInputElement & HTMLButtonElement>(null);

  // Each view starts with its one control focused.
  useEffect(() => {
    focusTarget.current?.focus();
  }, [step]);
  // The ready view may appear later than its step (after a step another
  // module adds has loaded), so it takes focus when it mounts.
  const focusOnMount = useCallback((node: HTMLButtonElement | null) => {
    node?.focus();
  }, []);

  const defaultName = t("create.defaultName");
  const finalName = name.trim() === "" ? defaultName : name.trim();

  const submit = (chosenKind: Kind, signupCode?: string) => {
    setFailed(false);
    create.mutate(
      {
        name: finalName,
        kind: chosenKind,
        ...(signupCode === undefined ? {} : { signup_code: signupCode }),
      },
      {
        onSuccess: (workspace) => {
          setCreated(workspace);
          setStep("done");
        },
        onError: (error) => {
          switch (problemCode(error)) {
            case "signup_code_invalid": {
              setCodeError(signupCode === undefined ? undefined : t("create.code.invalid"));
              setStep("code");
              break;
            }
            case "signup_invite_only": {
              setStep("inviteOnly");
              break;
            }
            case "signup_requires_approval": {
              setStep("approval");
              break;
            }
            default: {
              setFailed(true);
            }
          }
        },
      },
    );
  };

  const top = (
    <>
      <Brand name={tShell("appName")} />
      {firstRun || onCancel === undefined ? (
        <Button variant="ghost" icon="sign-out" onClick={onSignOut}>
          {t("create.signOut")}
        </Button>
      ) : (
        <Button variant="ghost" onClick={onCancel}>
          {t("create.cancel")}
        </Button>
      )}
    </>
  );
  const failure = failed ? (
    <Banner tone="danger" role="alert" title={t("create.failed.title")}>
      {t("create.failed.lead")}
    </Banner>
  ) : null;
  const accent = kind === null ? undefined : ACCENT_FOR_KIND[kind];
  const mark = <WorkspaceMark mark={initialsOf(finalName)} accent={accent} size="lg" />;
  const busy = create.isPending;

  switch (step) {
    case "name": {
      return (
        <FocusPage
          top={top}
          eyebrow={t("create.step", { step: 1, total: 2 })}
          title={firstRun ? t("create.name.title") : t("create.name.titleAnother")}
          lead={t("create.name.lead")}
          footer={firstRun ? t("create.required") : undefined}
          onSubmit={() => {
            if (name.trim().length > MAX_NAME_LENGTH) {
              setNameError(t("create.name.tooLong"));
              return;
            }
            setNameError(undefined);
            setStep("kind");
          }}
          actions={
            <>
              <Button type="submit" variant="primary" size="lg">
                {t("create.continue")}
              </Button>
              <Button
                variant="ghost"
                size="lg"
                onClick={() => {
                  setName("");
                  setNameError(undefined);
                  setStep("kind");
                }}
              >
                {t("create.skip")}
              </Button>
            </>
          }
        >
          <TextField
            ref={focusTarget}
            label={t("create.name.label")}
            hideLabel
            size="lg"
            autoComplete="organization"
            placeholder={t("create.name.placeholder")}
            maxLength={MAX_NAME_LENGTH + 20}
            value={name}
            hint={t("create.name.skipHint", { name: defaultName })}
            error={nameError}
            onChange={(event) => {
              setName(event.target.value);
            }}
          />
        </FocusPage>
      );
    }
    case "kind": {
      return (
        <FocusPage
          top={top}
          accent={accent}
          leading={mark}
          eyebrow={t("create.step", { step: 2, total: 2 })}
          title={t("create.kind.title")}
          lead={t("create.kind.lead")}
          onSubmit={() => {
            submit(kind ?? "business");
          }}
          actions={
            <>
              <Button type="submit" variant="primary" size="lg" disabled={busy}>
                {busy ? t("create.kind.creating") : t("create.kind.submit")}
              </Button>
              <Button
                variant="ghost"
                size="lg"
                disabled={busy}
                onClick={() => {
                  setFailed(false);
                  setStep("name");
                }}
              >
                {t("create.back")}
              </Button>
              <Button
                variant="ghost"
                size="lg"
                disabled={busy}
                onClick={() => {
                  setKind(null);
                  submit("business");
                }}
              >
                {t("create.skip")}
              </Button>
            </>
          }
        >
          <ChoiceCards
            label={t("create.kind.label")}
            hideLabel
            value={kind}
            onChange={setKind}
            options={[
              {
                value: "business",
                title: t("kind.business"),
                description: t("create.kind.business"),
              },
              {
                value: "personal",
                title: t("kind.personal"),
                description: t("create.kind.personal"),
              },
            ]}
          />
          {failure}
        </FocusPage>
      );
    }
    case "code": {
      return (
        <FocusPage
          top={top}
          title={t("create.code.title")}
          lead={t("create.code.lead")}
          onSubmit={() => {
            submit(kind ?? "business", code.trim());
          }}
          actions={
            <>
              <Button type="submit" variant="primary" size="lg" disabled={busy}>
                {busy ? t("create.kind.creating") : t("create.code.submit")}
              </Button>
              <Button
                variant="ghost"
                size="lg"
                disabled={busy}
                onClick={() => {
                  setCodeError(undefined);
                  setStep("kind");
                }}
              >
                {t("create.back")}
              </Button>
            </>
          }
        >
          <TextField
            ref={focusTarget}
            label={t("create.code.label")}
            size="lg"
            autoComplete="off"
            spellCheck={false}
            value={code}
            error={codeError}
            onChange={(event) => {
              setCode(event.target.value);
            }}
          />
          {failure}
        </FocusPage>
      );
    }
    case "inviteOnly": {
      return (
        <FocusPage
          top={top}
          leading={<Visor state="unsure" size="xl" />}
          title={t("create.inviteOnly.title")}
          lead={t("create.inviteOnly.lead")}
          actions={
            <Button ref={focusTarget} variant="secondary" size="lg" onClick={onSignOut}>
              {t("create.signOut")}
            </Button>
          }
        />
      );
    }
    case "approval": {
      return (
        <FocusPage
          top={top}
          leading={<Visor state="asleep" size="xl" />}
          title={t("create.approval.title")}
          lead={t("create.approval.lead")}
          actions={
            <Button
              ref={focusTarget}
              variant="secondary"
              size="lg"
              disabled={busy}
              onClick={() => {
                submit(kind ?? "business");
              }}
            >
              {t("create.approval.retry")}
            </Button>
          }
        >
          {failure}
        </FocusPage>
      );
    }
    case "done": {
      const workspace = created;
      if (workspace === null) {
        return null;
      }
      const ready = (
        <FocusPage
          top={<Brand name={tShell("appName")} />}
          accent={workspace.accent}
          leading={
            <WorkspaceMark mark={initialsOf(workspace.name)} accent={workspace.accent} size="lg" />
          }
          title={t("create.done.title", { name: workspace.name })}
          lead={t("create.done.lead")}
          actions={
            <>
              <Button
                ref={focusOnMount}
                variant="primary"
                size="lg"
                onClick={() => {
                  onOpen(workspace);
                }}
              >
                {t("create.done.open", { name: workspace.name })}
              </Button>
              <Button
                variant="secondary"
                size="lg"
                icon="people"
                onClick={() => {
                  onInvite(workspace);
                }}
              >
                {t("create.done.invite")}
              </Button>
            </>
          }
        />
      );
      return AfterCreate === undefined ? (
        ready
      ) : (
        <AfterCreate
          workspace={workspace}
          onContinue={() => {
            onOpen(workspace);
          }}
        >
          {ready}
        </AfterCreate>
      );
    }
  }
}
