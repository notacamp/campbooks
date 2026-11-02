import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { Button, SignatureEditor, Text, useUndoToast } from "~/lib/ui";

import { useSendingInboxes } from "../../api/drafts";
import {
  saveSignature,
  signatureKey,
  SIGNATURE_MAX_CHARS,
  useSignature,
} from "../../api/signatures";
import type { Translate } from "../../model";

export interface SignatureSettingsProps {
  workspaceId: string;
}

/**
 * Settings → Mailboxes → "Your signature": one editor for each mailbox the
 * member can send from. A signature is the member's own, per mailbox: in a
 * shared one each member signs with their own name.
 */
export function SignatureSettings({ workspaceId }: SignatureSettingsProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const inboxes = useSendingInboxes(workspaceId);
  if (inboxes.data === undefined || inboxes.data.length === 0) {
    return null;
  }
  return (
    <ul className="m-0 flex list-none flex-col gap-3 p-0" aria-label={t("signature.title")}>
      {inboxes.data.map((inbox) => (
        <li key={inbox.id}>
          <SignatureForm workspaceId={workspaceId} inboxId={inbox.id} address={inbox.address} />
        </li>
      ))}
    </ul>
  );
}

function SignatureForm({
  workspaceId,
  inboxId,
  address,
}: {
  workspaceId: string;
  inboxId: string;
  address: string;
}) {
  const saved = useSignature(workspaceId, inboxId);
  if (saved.data === undefined) {
    return (
      <Text tone="secondary" size="sm">
        {address}
      </Text>
    );
  }
  // The editor opens with what was kept and is not set again.
  return (
    <Editing workspaceId={workspaceId} inboxId={inboxId} address={address} start={saved.data} />
  );
}

function Editing({
  workspaceId,
  inboxId,
  address,
  start,
}: {
  workspaceId: string;
  inboxId: string;
  address: string;
  start: string;
}) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const queryClient = useQueryClient();
  const toast = useUndoToast();
  const [text, setText] = useState(start);
  const [kept, setKept] = useState(start);
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState<string | undefined>();
  const tooLong = text.length > SIGNATURE_MAX_CHARS;
  const changed = text !== kept;

  async function save() {
    if (saving || tooLong || !changed) {
      return;
    }
    setSaving(true);
    setFailed(undefined);
    try {
      const done = await saveSignature(workspaceId, inboxId, text);
      setKept(done);
      queryClient.setQueryData(signatureKey(workspaceId, inboxId), done);
      toast.notify(t("signature.saved"));
    } catch (thrown) {
      setFailed(
        problemCode(thrown) === "signature_too_long"
          ? t("signature.tooLong", { max: SIGNATURE_MAX_CHARS })
          : t("signature.failed"),
      );
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="flex flex-col gap-1" aria-label={t("signature.for", { address })}>
      <Text weight="semibold" size="sm">
        {t("signature.for", { address })}
      </Text>
      <SignatureEditor
        label={t("signature.for", { address })}
        initial={start}
        onChange={(marks) => {
          setText(marks);
          setFailed(undefined);
        }}
        onSubmit={() => {
          void save();
        }}
      />
      <div className="flex flex-wrap items-center gap-2">
        <Button
          variant="primary"
          disabled={saving || tooLong || !changed}
          onClick={() => {
            void save();
          }}
        >
          {t("signature.save")}
        </Button>
        <Text tone={tooLong ? "danger" : "secondary"} size="xs">
          {t("signature.chars", { chars: text.length, max: SIGNATURE_MAX_CHARS })}
        </Text>
        {failed === undefined && !tooLong ? null : (
          <span role="alert">
            <Text tone="danger" size="sm">
              {failed ?? t("signature.tooLong", { max: SIGNATURE_MAX_CHARS })}
            </Text>
          </span>
        )}
      </div>
    </section>
  );
}
