import { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  Heading,
  Icon,
  InlineConfirm,
  List,
  ListRow,
  SkeletonList,
  Text,
  TextField,
} from "~/lib/ui";

import {
  useAddPasskey,
  usePasskeys,
  useRemovePasskey,
  useRenamePasskey,
  type PasskeyView,
} from "../../api/queries";
import { leavePage, stepUpUrl } from "../../model";
import { ceremonyFailure, deviceName, passkeysSupported } from "../../webauthn";

export interface PasskeysSectionProps {
  /** Where a fresh sign-in comes back to (tests pass one; the page's own path otherwise). */
  returnTo?: string | undefined;
  /** Leaves the app for the sign-in (tests and stories pass a spy). */
  leave?: ((url: string) => void) | undefined;
}

/** Longest passkey name the server keeps. */
const MAX_NAME = 64;

function useDateFormat() {
  const { i18n } = useTranslation();
  const format = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });
  return (iso: string) => format.format(new Date(iso));
}

const here = () => `${globalThis.location.pathname}${globalThis.location.search}`;

/** What adding a passkey can come back with, as the section explains it. */
type AddProblem = "stepUp" | "cancelled" | "exists" | "expired" | "notAccepted" | "failed";

function addProblem(error: unknown): AddProblem {
  switch (problemCode(error)) {
    case "step_up_required":
      return "stepUp";
    case "passkey_expired":
      return "expired";
    case "passkey_not_accepted":
    case "passkey_already_registered":
      return "notAccepted";
    case undefined: {
      const failure = ceremonyFailure(error);
      return failure === "cancelled" || failure === "exists" ? failure : "failed";
    }
    default:
      return "failed";
  }
}

/**
 * Settings → Ways to sign in → Passkeys: the passkeys you sign in with
 * (Rename; Remove, confirmed in place, never the last way in) and "Add a
 * passkey" (`N P`), which asks the browser to make one. Adding or removing
 * needs a sign-in in the last ten minutes; the section says so and offers
 * one. Fetches its own data and takes no props in the app.
 */
export function PasskeysSection({ returnTo, leave = leavePage }: PasskeysSectionProps) {
  const { t } = useTranslation("identity");
  const titleId = useId();
  const passkeys = usePasskeys();
  const add = useAddPasskey();
  const [problem, setProblem] = useState<AddProblem | undefined>();
  const supported = passkeysSupported();
  const device = deviceName();

  const start = () => {
    if (add.isPending) {
      return;
    }
    setProblem(undefined);
    add.mutate(device === undefined ? undefined : t("passkeys.nameOn", { device }), {
      onError: (error) => {
        setProblem(addProblem(error));
      },
    });
  };
  useShortcut("identity.add_passkey", start, { enabled: supported && passkeys.isSuccess });
  const stepUp = () => {
    leave(stepUpUrl(returnTo ?? here()));
  };

  return (
    <section className="flex flex-col gap-3" aria-labelledby={titleId}>
      <Heading level={2} size="section" id={titleId}>
        {t("passkeys.title")}
      </Heading>
      <Text tone="tertiary" size="sm">
        {t("passkeys.lead")}
      </Text>
      {problem === "stepUp" ? (
        <StepUpBanner onSignIn={stepUp} />
      ) : problem === undefined ? null : (
        <Banner
          tone={problem === "cancelled" ? "neutral" : "danger"}
          role="alert"
          icon="key"
          title={t(`passkeys.problem.${problem}.title`)}
        >
          {t(`passkeys.problem.${problem}.lead`)}
        </Banner>
      )}
      {add.isSuccess && problem === undefined ? (
        <Banner role="status" icon="check" title={t("passkeys.added", { name: add.data.name })} />
      ) : null}
      {passkeys.isPending ? (
        <div role="status" aria-label={t("passkeys.loading")}>
          <SkeletonList rows={1} />
        </div>
      ) : passkeys.isError ? (
        <Banner
          tone="danger"
          role="alert"
          title={t("passkeys.loadError")}
          action={
            <Button
              variant="secondary"
              onClick={() => {
                void passkeys.refetch();
              }}
            >
              {t("passkeys.retry")}
            </Button>
          }
        />
      ) : passkeys.data.items.length === 0 ? (
        <Text tone="secondary" size="sm">
          {t("passkeys.none")}
        </Text>
      ) : (
        <List label={t("passkeys.list")}>
          {passkeys.data.items.map((passkey) => (
            <PasskeyRow
              key={passkey.id}
              passkey={passkey}
              onStepUp={() => {
                setProblem("stepUp");
              }}
            />
          ))}
        </List>
      )}
      {supported ? (
        <div>
          <Button
            variant="secondary"
            icon="plus"
            disabled={add.isPending || !passkeys.isSuccess}
            shortcut="identity.add_passkey"
            onClick={start}
          >
            {add.isPending ? t("passkeys.adding") : t("passkeys.add")}
          </Button>
        </div>
      ) : (
        <Text tone="tertiary" size="sm">
          {t("passkeys.unsupported")}
        </Text>
      )}
    </section>
  );
}

function StepUpBanner({ onSignIn }: { onSignIn: () => void }) {
  const { t } = useTranslation("identity");
  return (
    <Banner
      role="alert"
      icon="key"
      title={t("waysIn.stepUp.title")}
      action={
        <Button variant="secondary" onClick={onSignIn}>
          {t("waysIn.stepUp.action")}
        </Button>
      }
    >
      {t("waysIn.stepUp.lead")}
    </Banner>
  );
}

function PasskeyRow({ passkey, onStepUp }: { passkey: PasskeyView; onStepUp: () => void }) {
  const { t } = useTranslation("identity");
  const formatDate = useDateFormat();
  const remove = useRemovePasskey();
  const rename = useRenamePasskey();
  const [mode, setMode] = useState<"idle" | "renaming" | "confirming">("idle");
  const [error, setError] = useState<"lastWayIn" | "removeFailed" | "renameFailed" | undefined>();
  const meta = [
    passkey.synced ? t("passkeys.synced") : t("passkeys.thisDevice"),
    t("waysIn.added", { date: formatDate(passkey.created_at) }),
    passkey.last_used_at === null || passkey.last_used_at === undefined
      ? null
      : t("waysIn.lastUsed", { date: formatDate(passkey.last_used_at) }),
  ]
    .filter((part) => part !== null)
    .join(" · ");

  return (
    <ListRow
      leading={<Icon name="key" size="lg" />}
      title={passkey.name}
      meta={meta}
      trailing={
        mode === "idle" ? (
          <>
            <Button
              variant="ghost"
              aria-label={t("passkeys.renameLabel", { name: passkey.name })}
              onClick={() => {
                setError(undefined);
                setMode("renaming");
              }}
            >
              {t("passkeys.rename")}
            </Button>
            <Button
              variant="ghost"
              aria-label={t("passkeys.removeLabel", { name: passkey.name })}
              onClick={() => {
                setError(undefined);
                setMode("confirming");
              }}
            >
              {t("waysIn.remove")}
            </Button>
          </>
        ) : undefined
      }
    >
      {mode === "renaming" ? (
        <RenameForm
          initial={passkey.name}
          busy={rename.isPending}
          onCancel={() => {
            setMode("idle");
          }}
          onSave={(name) => {
            if (name === "" || name.length > MAX_NAME) {
              setError("renameFailed");
              return;
            }
            rename.mutate(
              { id: passkey.id, name },
              {
                onSuccess: () => {
                  setError(undefined);
                  setMode("idle");
                },
                onError: () => {
                  setError("renameFailed");
                },
              },
            );
          }}
        />
      ) : null}
      {mode === "confirming" ? (
        <InlineConfirm
          confirmLabel={t("waysIn.confirm")}
          cancelLabel={t("waysIn.cancel")}
          busy={remove.isPending}
          onCancel={() => {
            setMode("idle");
          }}
          onConfirm={() => {
            remove.mutate(passkey.id, {
              onError: (failure) => {
                setMode("idle");
                const code = problemCode(failure);
                if (code === "step_up_required") {
                  onStepUp();
                } else {
                  setError(code === "last_way_in" ? "lastWayIn" : "removeFailed");
                }
              },
            });
          }}
        >
          {t("passkeys.confirmRemove", { name: passkey.name })}
        </InlineConfirm>
      ) : null}
      {error === undefined ? null : (
        <Banner
          tone="danger"
          role="alert"
          title={
            error === "renameFailed" ? t("passkeys.renameFailed.title") : t(`waysIn.${error}.title`)
          }
        >
          {error === "renameFailed" ? t("passkeys.renameFailed.lead") : t(`waysIn.${error}.lead`)}
        </Banner>
      )}
    </ListRow>
  );
}

/** Rename in place: the name, Save (Enter) and Cancel; focused when it opens. */
function RenameForm({
  initial,
  busy,
  onSave,
  onCancel,
}: {
  initial: string;
  busy: boolean;
  onSave: (name: string) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation("identity");
  const field = useRef<HTMLInputElement>(null);
  const [name, setName] = useState(initial);
  useEffect(() => {
    field.current?.focus();
    field.current?.select();
  }, []);
  return (
    <form
      className="flex flex-col gap-2 sm:flex-row sm:items-end"
      onSubmit={(event) => {
        event.preventDefault();
        onSave(name.trim());
      }}
    >
      <TextField
        ref={field}
        label={t("passkeys.nameLabel")}
        value={name}
        maxLength={MAX_NAME + 20}
        onChange={(event) => {
          setName(event.target.value);
        }}
      />
      <div className="flex gap-2">
        <Button type="submit" variant="primary" disabled={busy}>
          {t("passkeys.save")}
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          {t("waysIn.cancel")}
        </Button>
      </div>
    </form>
  );
}
