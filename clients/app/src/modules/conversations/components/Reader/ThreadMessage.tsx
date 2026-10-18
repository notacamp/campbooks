import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  AttachmentList,
  Button,
  FoldedMessage,
  ImagePreview,
  MessageCard,
  SafeHtml,
  Skeleton,
  Text,
} from "~/lib/ui";

import { downloadAttachment, imageUrl, openPdf, type AttachmentRef } from "../../api/attachments";
import { useMessageBody, useMessageWords, type MessageView } from "../../api/queries";
import {
  attachmentKind,
  cleanFileName,
  displayName,
  messageTime,
  recipientsLine,
  type Translate,
} from "../../model";

export interface ThreadMessageProps {
  workspaceId: string;
  conversationId: string;
  message: MessageView;
  /** Folded to one line until opened. */
  open: boolean;
  onOpen: () => void;
  /** The member asked to see this message's remote images. */
  showImages: boolean;
  onShowImages: () => void;
  /**
   * "Always show images from <sender>": a smart rule for this sender
   * (campbooks-cloud#200), with the domain the server says may be trusted
   * instead; absent where the member may not teach rules. Offered only
   * where it can take effect (`images_offer`, campbooks-cloud#235).
   */
  onTrustImages?: ((domain: string | undefined) => void) | undefined;
  /** ⇧A trusts this message's sender (the newest message from someone else). */
  trustKey?: boolean | undefined;
  /** Other members use the inbox: a rule that trusts a sender is the inbox's. */
  sharedInbox?: boolean | undefined;
  kind: "person" | "service";
  /** The conversation may be phishing: the body's links lead nowhere. */
  inertLinks?: boolean | undefined;
  /** Show the reader's keys (⇧I, V) on its buttons; off where they are not bound. */
  shortcuts?: boolean | undefined;
  now?: Date | undefined;
}

/**
 * One message of the thread: its plain text, or its HTML body (sanitised by
 * the server) in the sandboxed frame, with "Show images" when remote images
 * were held back, and its attachments.
 */
export function ThreadMessage({
  workspaceId,
  conversationId,
  message,
  open,
  onOpen,
  showImages,
  onShowImages,
  onTrustImages,
  trustKey = false,
  sharedInbox = false,
  kind,
  inertLinks = false,
  shortcuts = true,
  now,
}: ThreadMessageProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const body = useMessageBody(
    workspaceId,
    conversationId,
    message.id,
    showImages,
    open && message.has_html,
  );
  const from = displayName(message.from);
  const time = messageTime(message.sent_at, i18n.language, now);
  // Its words for the folded snippet, asked for on their own when the
  // conversation came without them (`text_unread`); open, it shows its HTML.
  const unread = useMessageWords(workspaceId, conversationId, open ? undefined : message);
  const text = message.text?.content ?? unread.data?.content ?? "";
  const files = useAttachments(workspaceId, conversationId, message, t, shortcuts);
  // Trusting the sender, only where the server says it can take effect.
  const domain = body.data?.images_domain ?? undefined;
  const trust =
    onTrustImages !== undefined && body.data?.images_offer === true
      ? () => {
          onTrustImages(domain);
        }
      : undefined;
  useShortcut(
    "inbox.trust_images",
    () => {
      trust?.();
    },
    { enabled: trustKey && shortcuts && trust !== undefined },
  );

  if (!open) {
    return (
      <FoldedMessage
        from={from}
        avatar={{ kind }}
        time={time}
        dateTime={message.sent_at}
        snippet={
          message.text_unread === true && unread.isPending
            ? t("reader.readingText")
            : text.replaceAll(/\s+/gu, " ").trim()
        }
        onOpen={onOpen}
      />
    );
  }

  const remote = body.data?.remote_images ?? 0;
  const html = body.data?.html;

  let content;
  if (!message.has_html) {
    content = text;
  } else if (body.isPending) {
    content = (
      <span className="flex flex-col gap-2" aria-busy="true">
        <Skeleton width="90%" />
        <Skeleton width="70%" />
        <Skeleton width="80%" />
      </span>
    );
  } else if (html === undefined || html === null) {
    content = body.isError ? <Text tone="tertiary">{t("error.body")}</Text> : text;
  } else {
    content = (
      <SafeHtml
        html={html}
        title={t("reader.frameTitle", { name: from })}
        remoteImages={body.data?.images_shown ?? false}
        css={body.data?.css}
        inertLinks={inertLinks}
      />
    );
  }

  const truncated = message.text?.truncated === true || body.data?.truncated === true;
  return (
    <MessageCard
      from={from}
      avatar={{ kind }}
      time={time}
      dateTime={message.sent_at}
      recipients={recipientsLine(t, message, i18n.language)}
      rich={message.has_html}
      files={files}
      actions={
        remote > 0 || truncated ? (
          <>
            {truncated ? (
              <Text tone="tertiary" size="sm">
                {t("reader.truncated")}
              </Text>
            ) : null}
            {remote > 0 && body.data?.images_shown !== true ? (
              <>
                <Text tone="tertiary" size="sm">
                  {t("actions.imagesHidden", { count: remote })}
                </Text>
                <Button
                  variant="secondary"
                  icon="image"
                  shortcut={shortcuts ? "inbox.show_images" : undefined}
                  onClick={onShowImages}
                >
                  {t("actions.showImages")}
                </Button>
                {trust === undefined ? null : (
                  <Button
                    variant="secondary"
                    shortcut={shortcuts && trustKey ? "inbox.trust_images" : undefined}
                    onClick={trust}
                  >
                    {t("actions.trustImages", { name: from })}
                  </Button>
                )}
                {body.data?.images_held === undefined || body.data.images_held === null ? null : (
                  <Text tone="tertiary" size="sm">
                    {t(`actions.imagesHeld.${body.data.images_held}`)}
                  </Text>
                )}
              </>
            ) : null}
            {remote > 0 && body.data?.images_trusted === true ? (
              <Text tone="tertiary" size="sm">
                {t(sharedInbox ? "actions.imagesTrustedInbox" : "actions.imagesTrusted", {
                  name: from,
                })}
              </Text>
            ) : null}
          </>
        ) : undefined
      }
    >
      {content}
    </MessageCard>
  );
}

/**
 * The message's files (not the images its body shows) with Download and
 * Preview: a picture in a dialog from a `blob:` URL, a PDF in the browser's
 * viewer in a new tab, anything else only downloaded.
 */
function useAttachments(
  workspaceId: string,
  conversationId: string,
  message: MessageView,
  t: Translate,
  shortcuts: boolean,
) {
  const [busy, setBusy] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [preview, setPreview] = useState<{ id: string; src: string | undefined } | null>(null);
  const attachments = message.attachments.filter((attachment) => !attachment.in_body);

  // Let go of the picture's bytes when the preview closes or changes.
  const src = preview?.src;
  useEffect(
    () => () => {
      if (src !== undefined) {
        URL.revokeObjectURL(src);
      }
    },
    [src],
  );

  if (attachments.length === 0) {
    return;
  }
  const refOf = (attachmentId: string): AttachmentRef => ({
    workspaceId,
    conversationId,
    messageId: message.id,
    attachmentId,
  });
  const find = (id: string) => attachments.find((attachment) => attachment.id === id);
  const fail = (id: string) => {
    setError(t("error.attachment", { name: cleanFileName(find(id)?.filename ?? "") }));
  };

  const download = (id: string) => {
    const attachment = find(id);
    if (attachment === undefined) {
      return;
    }
    setError(undefined);
    setBusy(id);
    downloadAttachment(refOf(id), attachment.filename)
      .catch(() => {
        fail(id);
      })
      .finally(() => {
        setBusy(undefined);
      });
  };
  const showPreview = (id: string) => {
    const attachment = find(id);
    if (attachment === undefined) {
      return;
    }
    setError(undefined);
    if (attachmentKind(attachment.content_type) === "pdf") {
      if (!openPdf(refOf(id))) {
        download(id);
      }
      return;
    }
    setPreview({ id, src: undefined });
    imageUrl(refOf(id), attachment.content_type)
      .then((url) => {
        setPreview((current) => {
          if (current?.id !== id) {
            URL.revokeObjectURL(url);
            return current;
          }
          return { id, src: url };
        });
      })
      .catch(() => {
        setPreview(null);
        fail(id);
      });
  };

  const previewed = preview === null ? undefined : find(preview.id);
  return (
    <>
      <AttachmentList
        attachments={attachments.map((attachment) => ({
          id: attachment.id,
          name: cleanFileName(attachment.filename),
          size: attachment.size,
          kind: attachmentKind(attachment.content_type),
        }))}
        busy={busy}
        error={error}
        shortcut={shortcuts ? "inbox.next_attachment" : undefined}
        onDownload={download}
        onPreview={showPreview}
      />
      {preview === null || previewed === undefined ? null : (
        <ImagePreview
          src={preview.src}
          name={cleanFileName(previewed.filename)}
          onClose={() => {
            setPreview(null);
          }}
          onDownload={() => {
            download(previewed.id);
          }}
        />
      )}
    </>
  );
}
