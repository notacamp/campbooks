import { useRouter } from "@tanstack/react-router";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, loginUrl, useSession } from "~/lib/api";
import { Banner, Brand, Button, FocusPage, Visor } from "~/lib/ui";

import { useSignInRequest } from "../../api/queries";
import {
  leavePage,
  rememberedReturnTo,
  rememberReturnTo,
  safeReturnTo,
  signInError,
  startAgainUrl,
} from "../../model";

import { SignInChoices } from "./SignIn";

export interface SignInScreenProps {
  /** The waiting authorization request, when the issuer sent the browser here. */
  request?: string | undefined;
  /** The callback's error code. */
  error?: string | undefined;
  /** Where to come back to, when the app sent the browser here. */
  returnTo?: string | undefined;
  /** Full-page navigation (tests pass a spy). */
  leave?: ((url: string) => void) | undefined;
}

/**
 * `/sign-in`. With a `request`, the ways in for it. Without one, it starts a
 * sign-in through the BFF (remembering where to come back to), unless the
 * browser is already signed in or a callback error needs explaining first.
 */
export function SignInScreen({ request, error, returnTo, leave = leavePage }: SignInScreenProps) {
  return request === undefined ? (
    <StartSignIn error={error} returnTo={safeReturnTo(returnTo)} leave={leave} />
  ) : (
    <WaitingRequest request={request} error={error} leave={leave} />
  );
}

function Waiting({ title }: { title: string }) {
  const { t } = useTranslation("shell");
  return (
    <FocusPage
      top={<Brand name={t("appName")} />}
      leading={<Visor state="thinking" size="xl" announce />}
      title={title}
    />
  );
}

/** A sign-in that can't go on from here: what happened, and Start again. */
function StartAgain({
  title,
  lead,
  banner,
  leave,
}: {
  title: string;
  lead: string;
  banner?: { title: string; lead: string } | undefined;
  leave: (url: string) => void;
}) {
  const { t } = useTranslation("identity");
  const { t: tShell } = useTranslation("shell");
  return (
    <FocusPage
      top={<Brand name={tShell("appName")} />}
      leading={<Visor state="unsure" size="xl" />}
      // Error sentences end with a full stop; a page title doesn't.
      title={title.replace(/\.$/u, "")}
      lead={lead}
      actions={
        <Button
          variant="primary"
          size="lg"
          onClick={() => {
            leave(startAgainUrl());
          }}
        >
          {t("signIn.startAgain")}
        </Button>
      }
    >
      {banner === undefined ? undefined : (
        <Banner tone="danger" role="alert" title={banner.title}>
          {banner.lead}
        </Banner>
      )}
    </FocusPage>
  );
}

function StartSignIn({
  error,
  returnTo,
  leave,
}: {
  error: string | undefined;
  returnTo: string;
  leave: (url: string) => void;
}) {
  const { t } = useTranslation("identity");
  const router = useRouter();
  const session = useSession();
  const signedIn = session.data?.authenticated === true;
  const starting = error === undefined && session.isSuccess && !signedIn;

  useEffect(() => {
    if (error === undefined && signedIn) {
      router.history.replace(returnTo);
    }
  }, [error, signedIn, returnTo, router]);
  useEffect(() => {
    if (starting) {
      rememberReturnTo(returnTo);
      leave(loginUrl(returnTo));
    }
  }, [starting, returnTo, leave]);

  if (error !== undefined) {
    const problem = signInError(error);
    return (
      <StartAgain
        title={t(`errors.${problem}.title`)}
        lead={t(`errors.${problem}.lead`)}
        leave={leave}
      />
    );
  }
  if (session.isError) {
    // Can't tell whether the browser is signed in: try signing in anyway.
    return (
      <StartAgain title={t("signIn.failed.title")} lead={t("signIn.failed.lead")} leave={leave} />
    );
  }
  return <Waiting title={t("signIn.starting")} />;
}

function WaitingRequest({
  request,
  error,
  leave,
}: {
  request: string;
  error: string | undefined;
  leave: (url: string) => void;
}) {
  const { t } = useTranslation("identity");
  const { t: tShell } = useTranslation("shell");
  const info = useSignInRequest(request);

  if (info.isPending) {
    return <Waiting title={t("signIn.loading")} />;
  }
  if (info.isError) {
    if (errorStatus(info.error) === 404) {
      const problem = error === undefined ? undefined : signInError(error);
      return (
        <StartAgain
          title={t("signIn.expired.title")}
          lead={t("signIn.expired.lead")}
          banner={
            problem === undefined
              ? undefined
              : { title: t(`errors.${problem}.title`), lead: t(`errors.${problem}.lead`) }
          }
          leave={leave}
        />
      );
    }
    return (
      <FocusPage
        top={<Brand name={tShell("appName")} />}
        leading={<Visor state="unsure" size="xl" />}
        title={t("signIn.failed.title")}
        lead={t("signIn.failed.lead")}
        actions={
          <Button
            variant="primary"
            size="lg"
            onClick={() => {
              void info.refetch();
            }}
          >
            {t("signIn.retry")}
          </Button>
        }
      />
    );
  }
  return (
    <SignInChoices
      request={request}
      providers={info.data.providers}
      error={error}
      returnTo={rememberedReturnTo()}
      leave={leave}
    />
  );
}
