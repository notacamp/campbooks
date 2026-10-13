import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Banner, Brand, ButtonLink, Disclosure, FocusPage, Text, Visor } from "~/lib/ui";

import {
  CHOICE_SHORTCUTS,
  isInvitation,
  leavePage,
  signInError,
  startUrl,
  waysIn,
  type Provider,
} from "../../model";

export interface SignInChoicesProps {
  /** The waiting authorization request (`?request=`). */
  request: string;
  /** The ways in the server lists for it (`RequestInfo.providers`). */
  providers: readonly string[];
  /** The callback's `?error=`, explained above the ways in. */
  error?: string | undefined;
  /** Where signing in comes back to (picks the invitation copy). */
  returnTo: string;
  /** Leaves the app for a provider (tests and stories pass a spy). */
  leave?: ((url: string) => void) | undefined;
}

/** A provider's button label key, typed for `t()`. */
const WITH = {
  google: "signIn.with.google",
  microsoft: "signIn.with.microsoft",
  zoho: "signIn.with.zoho",
} as const satisfies Record<Provider, string>;

/**
 * `/sign-in?request=`: the brand and one line, then every way in the server
 * offers (`1`, `2`, `3`, and `P` for a passkey). With Google, it says before
 * anyone leaves what the one consent covers, and that Google may warn the app
 * isn't verified yet.
 */
export function SignInChoices({
  request,
  providers,
  error,
  returnTo,
  leave = leavePage,
}: SignInChoicesProps) {
  const { t } = useTranslation("identity");
  const { t: tShell } = useTranslation("shell");
  const ways = waysIn(providers);
  const href = (index: number) => {
    const provider = ways.providers[index];
    return provider === undefined ? undefined : startUrl(provider, request);
  };
  const go = (url: string | undefined) => () => {
    if (url !== undefined) {
      leave(url);
    }
  };
  // One binding per position: hooks can't be called in a loop.
  useShortcut(CHOICE_SHORTCUTS[0], go(href(0)), { enabled: href(0) !== undefined });
  useShortcut(CHOICE_SHORTCUTS[1], go(href(1)), { enabled: href(1) !== undefined });
  useShortcut(CHOICE_SHORTCUTS[2], go(href(2)), { enabled: href(2) !== undefined });
  const passkeyHref = ways.passkey ? startUrl("passkey", request) : undefined;
  useShortcut("sign_in.passkey", go(passkeyHref), { enabled: ways.passkey });

  const invited = isInvitation(returnTo);
  const none = ways.providers.length === 0 && !ways.passkey;
  const problem = error === undefined ? undefined : signInError(error);

  return (
    <FocusPage
      top={<Brand name={tShell("appName")} />}
      leading={<Visor size="xl" state={problem === undefined ? "idle" : "unsure"} />}
      title={
        none ? t("signIn.noWayIn.title") : invited ? t("signIn.invitedTitle") : t("signIn.title")
      }
      lead={none ? t("signIn.noWayIn.lead") : invited ? t("signIn.invitedLead") : t("signIn.lead")}
      footer={t("signIn.footer")}
    >
      {problem === undefined ? undefined : (
        <Banner tone="danger" role="alert" title={t(`errors.${problem}.title`)}>
          {t(`errors.${problem}.lead`)}
        </Banner>
      )}
      {none ? undefined : (
        <div className="flex flex-col gap-2" role="group" aria-label={t("signIn.waysIn")}>
          {ways.providers.map((provider, index) => (
            <ButtonLink
              key={provider}
              className="w-full"
              variant={index === 0 ? "primary" : "secondary"}
              size="lg"
              href={startUrl(provider, request)}
              shortcut={CHOICE_SHORTCUTS[index]}
            >
              {t(WITH[provider])}
            </ButtonLink>
          ))}
          {passkeyHref === undefined ? null : (
            <ButtonLink
              className="w-full"
              variant="ghost"
              size="lg"
              icon="key"
              href={passkeyHref}
              shortcut="sign_in.passkey"
            >
              {t("signIn.passkey")}
            </ButtonLink>
          )}
        </div>
      )}
      {ways.providers.includes("google") ? <GoogleConsent /> : undefined}
    </FocusPage>
  );
}

/** What signing in with Google also asks for, read before leaving for Google. */
function GoogleConsent() {
  const { t } = useTranslation("identity");
  return (
    <div className="flex flex-col gap-3">
      <Text tone="secondary" size="sm">
        {t("signIn.gmail.summary")}
      </Text>
      <Disclosure summary={t("signIn.gmail.details")}>
        <p>{t("signIn.gmail.keeps")}</p>
        <p>{t("signIn.gmail.reads")}</p>
        <p>{t("signIn.gmail.optional")}</p>
        <p>{t("signIn.gmail.existing")}</p>
      </Disclosure>
      <Text tone="tertiary" size="sm">
        {t("signIn.unverified")}
      </Text>
    </div>
  );
}
