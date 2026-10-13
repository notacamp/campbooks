import { useEffect, useRef, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  Avatar,
  Banner,
  Brand,
  Button,
  ButtonLink,
  FocusPage,
  List,
  ListRow,
  Steps,
  Text,
  Visor,
} from "~/lib/ui";

import { useCreateAccount, useNewAccount, type NewAccount } from "../../api/queries";
import { leavePage, startAgainUrl } from "../../model";

export interface NewAccountScreenProps {
  /** The waiting authorization request, to go back to the ways in. */
  request?: string | undefined;
  /** Full-page navigation (tests pass a spy). */
  leave?: ((url: string) => void) | undefined;
}

/** The provider's name as people know it ("Google"); unknown ones as given. */
function useProviderName() {
  const { t } = useTranslation("identity");
  return (provider: string) =>
    provider === "google" || provider === "microsoft" || provider === "zoho"
      ? t(`provider.${provider}`)
      : provider;
}

/**
 * `/sign-in/new`: no Campbooks account uses this provider account yet. Shows
 * the name and address that will be used and what happens next; Create
 * (Enter) or sign in another way (Esc). Never says whether another account
 * uses the address.
 */
export function NewAccountScreen({ request, leave = leavePage }: NewAccountScreenProps) {
  const { t } = useTranslation("identity");
  const pending = useNewAccount();

  if (pending.isPending) {
    return <Simple visor="thinking" title={t("newAccount.loading")} announce />;
  }
  if (pending.isError) {
    if (errorStatus(pending.error) === 404) {
      return <Gone leave={leave} />;
    }
    return (
      <Simple
        visor="unsure"
        title={t("newAccount.loadFailed.title")}
        lead={t("newAccount.loadFailed.lead")}
        action={
          <Button
            variant="primary"
            size="lg"
            onClick={() => {
              void pending.refetch();
            }}
          >
            {t("newAccount.retry")}
          </Button>
        }
      />
    );
  }
  return <CreateAccount account={pending.data} request={request} leave={leave} />;
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

/** The sign-up is no longer waiting for this browser. */
function Gone({ leave }: { leave: (url: string) => void }) {
  const { t } = useTranslation("identity");
  return (
    <Simple
      visor="unsure"
      title={t("newAccount.gone.title")}
      lead={t("newAccount.gone.lead")}
      action={
        <Button
          variant="primary"
          size="lg"
          onClick={() => {
            leave(startAgainUrl());
          }}
        >
          {t("newAccount.signInAgain")}
        </Button>
      }
    />
  );
}

function CreateAccount({
  account,
  request,
  leave,
}: {
  account: NewAccount;
  request: string | undefined;
  leave: (url: string) => void;
}) {
  const { t } = useTranslation("identity");
  const { t: tShell } = useTranslation("shell");
  const providerName = useProviderName();
  const create = useCreateAccount();
  const submit = useRef<HTMLButtonElement>(null);
  const provider = providerName(account.provider);
  const name = account.display_name ?? account.email ?? provider;
  const backHref =
    request === undefined ? startAgainUrl() : `/sign-in?request=${encodeURIComponent(request)}`;

  useEffect(() => {
    submit.current?.focus();
  }, []);
  useShortcut("overlay.close", () => {
    leave(backHref);
  });

  const code = create.isError ? problemCode(create.error) : undefined;
  if (code === "sign_up_expired" || (create.isError && errorStatus(create.error) === 404)) {
    return <Gone leave={leave} />;
  }
  const takenElsewhere = code === "way_in_belongs_to_another_account";
  const busy = create.isPending || create.isSuccess;

  return (
    <FocusPage
      top={<Brand name={tShell("appName")} />}
      leading={<Visor size="xl" state="happy" />}
      eyebrow={t("newAccount.eyebrow")}
      title={t("newAccount.title")}
      lead={
        account.email === null || account.email === undefined
          ? t("newAccount.leadNoEmail", { provider })
          : t("newAccount.lead", { email: account.email })
      }
      footer={t("newAccount.footer")}
      formLabel={t("newAccount.title")}
      onSubmit={() => {
        if (busy) {
          return;
        }
        create.mutate(undefined, {
          onSuccess: (redirect) => {
            leave(redirect);
          },
        });
      }}
      actions={
        <>
          <Button
            ref={submit}
            type="submit"
            variant="primary"
            size="lg"
            keys="enter"
            disabled={busy}
          >
            {busy ? t("newAccount.creating") : t("newAccount.create")}
          </Button>
          <ButtonLink variant="ghost" size="lg" href={backHref} shortcut="overlay.close">
            {t("newAccount.another")}
          </ButtonLink>
        </>
      }
    >
      <List label={t("newAccount.identity")}>
        <ListRow
          leading={<Avatar name={name} size="lg" />}
          title={name}
          badge={t("newAccount.from", { provider })}
          meta={account.display_name === null ? undefined : (account.email ?? undefined)}
        />
      </List>
      <Text tone="tertiary" size="sm">
        {t("newAccount.nameNote", { provider })}
      </Text>
      <Steps
        label={t("newAccount.nextTitle")}
        steps={[t("newAccount.next1", { provider }), t("newAccount.next2"), t("newAccount.next3")]}
      />
      {create.isError ? (
        <Banner
          tone="danger"
          role="alert"
          title={
            takenElsewhere
              ? t("errors.way_in_belongs_to_another_account.title")
              : t("newAccount.failed.title")
          }
        >
          {takenElsewhere
            ? t("errors.way_in_belongs_to_another_account.lead")
            : t("newAccount.failed.lead")}
        </Banner>
      ) : undefined}
    </FocusPage>
  );
}
