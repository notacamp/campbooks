import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Banner, Button, Heading, InlineConfirm, Text } from "~/lib/ui";

import { useAdoptHeldGrant, useDiscardHeldGrant, type HeldGrant } from "../../api/queries";
import { adoptFailure, useWaitsUntil } from "../MailboxStep/MailboxStep";

export interface MailboxOfferProps {
  /** The mailbox held since sign-up. */
  grant: HeldGrant;
  /** The open workspace, where Connect puts it. */
  workspaceId: string;
  workspaceName: string;
}

/**
 * Home's mail section while a mailbox from sign-up is waiting: Connect it to
 * this workspace, or forget it (confirmed in place: it can't be undone).
 */
export function MailboxOffer({ grant, workspaceId, workspaceName }: MailboxOfferProps) {
  const { t } = useTranslation("identity");
  const adopt = useAdoptHeldGrant(workspaceId);
  const discard = useDiscardHeldGrant();
  const [confirming, setConfirming] = useState(false);
  const until = useWaitsUntil(grant);
  const failure = adopt.isError ? adoptFailure(adopt.error) : undefined;
  const busy = adopt.isPending || discard.isPending;

  return (
    <section className="flex flex-col items-start gap-3" aria-labelledby="held-mailbox">
      <Heading level={2} size="title" id="held-mailbox">
        {t("offer.title", { address: grant.address })}
      </Heading>
      <Text tone="secondary">{t("offer.lead", { workspace: workspaceName })}</Text>
      <Text tone="tertiary" size="sm">
        {t("offer.waits", { date: until })}
      </Text>
      {failure === undefined ? undefined : (
        <Banner tone="danger" role="alert" title={t(`mailbox.errors.${failure}.title`)}>
          {t(`mailbox.errors.${failure}.lead`, { address: grant.address })}
        </Banner>
      )}
      {discard.isError ? (
        <Banner tone="danger" role="alert" title={t("offer.forgetFailed")} />
      ) : undefined}
      {confirming ? (
        <InlineConfirm
          confirmLabel={t("offer.confirm")}
          cancelLabel={t("offer.cancel")}
          busy={discard.isPending}
          onCancel={() => {
            setConfirming(false);
          }}
          onConfirm={() => {
            discard.mutate(grant.id, {
              onSettled: () => {
                setConfirming(false);
              },
            });
          }}
        >
          {t("offer.confirmForget", { address: grant.address })}
        </InlineConfirm>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="primary"
            size="lg"
            icon="mail"
            disabled={busy}
            onClick={() => {
              adopt.mutate(grant.id);
            }}
          >
            {adopt.isPending ? t("mailbox.connecting") : t("offer.connect")}
          </Button>
          <Button
            variant="ghost"
            size="lg"
            disabled={busy}
            onClick={() => {
              setConfirming(true);
            }}
          >
            {t("offer.forget")}
          </Button>
        </div>
      )}
    </section>
  );
}
