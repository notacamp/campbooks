import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import { Banner, Brand, Button, ButtonLink, FocusPage, Steps, TextField, Visor } from "~/lib/ui";

import { usePasskeySignUp } from "../../api/queries";
import { leavePage, startAgainUrl } from "../../model";
import { ceremonyFailure, passkeysSupported } from "../../webauthn";

export interface PasskeySignUpScreenProps {
  /** The waiting authorization request, completed once the account exists. */
  request?: string | undefined;
  /** Full-page navigation (tests pass a spy). */
  leave?: ((url: string) => void) | undefined;
}

/** Longest name the server keeps. */
const MAX_NAME = 64;

/** Why creating the account didn't go through, as this screen explains it. */
type Problem = "cancelled" | "exists" | "expired" | "not_accepted" | "failed";

function problemOf(error: unknown): Problem | "invite_only" | "name" {
  switch (problemCode(error)) {
    case "signup_invite_only":
      return "invite_only";
    case "display_name_required":
      return "name";
    case "passkey_expired":
      return "expired";
    case "passkey_not_accepted":
    case "passkey_already_registered":
      return "not_accepted";
    case undefined: {
      const failure = ceremonyFailure(error);
      return failure === "cancelled" || failure === "exists" ? failure : "failed";
    }
    default:
      return "failed";
  }
}

/**
 * `/sign-in/passkey`: a new account with a passkey alone, no provider. One
 * question (what to call you), then the browser makes the passkey; Create
 * (Enter) or sign in another way (Esc). An invite-only instance refuses it:
 * a passkey proves no address an invitation could match.
 */
export function PasskeySignUpScreen({ request, leave = leavePage }: PasskeySignUpScreenProps) {
  const { t } = useTranslation("identity");
  const { t: tShell } = useTranslation("shell");
  const signUp = usePasskeySignUp();
  const field = useRef<HTMLInputElement>(null);
  const [name, setName] = useState("");
  const [nameError, setNameError] = useState<string | undefined>();
  const backHref =
    request === undefined ? startAgainUrl() : `/sign-in?request=${encodeURIComponent(request)}`;

  useEffect(() => {
    field.current?.focus();
  }, []);
  useShortcut("overlay.close", () => {
    leave(backHref);
  });

  const failure = signUp.isError ? problemOf(signUp.error) : undefined;
  const top = <Brand name={tShell("appName")} />;
  if (!passkeysSupported() || failure === "invite_only") {
    const reason = failure === "invite_only" ? "inviteOnly" : "unsupported";
    return (
      <FocusPage
        top={top}
        leading={<Visor state="unsure" size="xl" />}
        title={t(`passkeySignUp.${reason}.title`)}
        lead={t(`passkeySignUp.${reason}.lead`)}
        actions={
          <ButtonLink variant="primary" size="lg" href={backHref}>
            {t("passkeySignUp.another")}
          </ButtonLink>
        }
      />
    );
  }
  const busy = signUp.isPending || signUp.isSuccess;
  const problem = failure === undefined || failure === "name" ? undefined : failure;

  return (
    <FocusPage
      top={top}
      leading={<Visor size="xl" state={busy ? "thinking" : "happy"} />}
      eyebrow={t("passkeySignUp.eyebrow")}
      title={t("passkeySignUp.title")}
      lead={t("passkeySignUp.lead")}
      footer={t("passkeySignUp.footer")}
      formLabel={t("passkeySignUp.title")}
      onSubmit={() => {
        if (busy) {
          return;
        }
        const trimmed = name.trim();
        if (trimmed === "" || trimmed.length > MAX_NAME) {
          setNameError(
            trimmed === "" ? t("passkeySignUp.name.required") : t("passkeySignUp.name.tooLong"),
          );
          field.current?.focus();
          return;
        }
        setNameError(undefined);
        signUp.mutate(
          { display_name: trimmed, ...(request === undefined ? {} : { request }) },
          {
            onSuccess: (redirect) => {
              leave(redirect);
            },
          },
        );
      }}
      actions={
        <>
          <Button type="submit" variant="primary" size="lg" icon="key" keys="enter" disabled={busy}>
            {busy ? t("passkeySignUp.creating") : t("passkeySignUp.create")}
          </Button>
          <ButtonLink variant="ghost" size="lg" href={backHref} shortcut="overlay.close">
            {t("passkeySignUp.another")}
          </ButtonLink>
        </>
      }
    >
      <TextField
        ref={field}
        label={t("passkeySignUp.name.label")}
        size="lg"
        autoComplete="name"
        placeholder={t("passkeySignUp.name.placeholder")}
        maxLength={MAX_NAME + 20}
        value={name}
        hint={t("passkeySignUp.name.hint")}
        error={failure === "name" ? t("passkeySignUp.name.required") : nameError}
        disabled={busy}
        onChange={(event) => {
          setName(event.target.value);
        }}
      />
      <Steps
        label={t("passkeySignUp.nextTitle")}
        steps={[t("passkeySignUp.next1"), t("passkeySignUp.next2"), t("passkeySignUp.next3")]}
      />
      {problem === undefined ? null : (
        <Banner
          tone={problem === "cancelled" ? "neutral" : "danger"}
          role="alert"
          icon="key"
          title={t(`passkeySignUp.problem.${problem}.title`)}
        >
          {t(`passkeySignUp.problem.${problem}.lead`)}
        </Banner>
      )}
    </FocusPage>
  );
}
