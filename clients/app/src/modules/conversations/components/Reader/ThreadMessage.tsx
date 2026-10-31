import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  AttachmentList,
  Button,
  FoldedMessage,
  ImagePreview,
  initialsOf,
  MessageCard,
  QuotedText,
  SafeHtml,
  Skeleton,
  Text,
} from "~/lib/ui";

import { downloadAttachment, imageUrl, openPdf, type AttachmentRef } from "../../api/attachments";
import { useRemoteImagesAllowed } from "../../api/body-freshness";
import { useMessageBody, useMessageWords, type MessageView } from "../../api/queries";
import {
  addresseeLine,
  attachmentKind,
  cleanFileName,
  displayName,
  messageTime,
  recipientsLine,
  splitQuoted,
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
  /** Its conversation is in spam: remote images never load. */
  spam?: boolean | undefined;
  /** Show the reader's keys (⇧I, V) on its buttons; off where they are not bound. */
  shortcuts?: boolean | undefined;
  /** What was read from it (the calendar's strip), under its header; shown while open. */
  found?: ReactNode | undefined;
  now?: Date | undefined;
}

/**
 * One message of the thread: its plain text, or its HTML body (sanitised by
 * the server) in the sandboxed frame, with "Show images" when remote images
 * were held back, and its attachments. Its quoted history is folded behind
 * "Show quoted text"; a message the inbox sent reads "You → <recipient>",
 * set in from the left.
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
  spam = false,
  shortcuts = true,
  found,
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
    spam,
  );
  // Without a live feed, a trusted sender's images wait for a fresh read
  // (campbooks-cloud#236): the cached body shows with them held back.
  const imagesAllowed = useRemoteImagesAllowed(body, open, showImages);
  const imagesChecking = body.data?.images_shown === true && !imagesAllowed;
  const sender = displayName(message.from);
  const yours = message.direction === "outbound";
  const from = yours ? t("reader.you") : sender;
  const side = yours ? "yours" : "theirs";
  const addressee = yours ? addresseeLine(t, message) : undefined;
  const avatar = yours ? { kind, initials: initialsOf(sender) } : { kind };
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
        anchor={`message-${message.id}`}
        from={from}
        addressee={addressee}
        side={side}
        avatar={avatar}
        time={time}
        dateTime={message.sent_at}
        snippet={
          message.text_unread !== true || unread.data !== undefined
            ? text.replaceAll(/\s+/gu, " ").trim()
            : unread.isError
              ? t("reader.textFailed")
              : t("reader.readingText")
        }
        onOpen={onOpen}
      />
    );
  }

  const remote = body.data?.remote_images ?? 0;
  const html = body.data?.html;

  const plain = splitQuoted(text);
  let content;
  // A plain-text body's quoted history, folded under its own words.
  let quoted: string | undefined;
  if (!message.has_html) {
    content = plain.own;
    quoted = plain.quoted;
  } else if (body.isPending) {
    content = (
      <span className="flex flex-col gap-2" aria-busy="true">
        <Skeleton width="90%" />
        <Skeleton width="70%" />
        <Skeleton width="80%" />
      </span>
    );
  } else if (html === undefined || html === null) {
    if (body.isError) {
      content = <Text tone="tertiary">{t("error.body")}</Text>;
    } else {
      content = plain.own;
      quoted = plain.quoted;
    }
  } else {
    content = (
      <SafeHtml
        html={html}
        title={t("reader.frameTitle", { name: sender })}
        remoteImages={imagesAllowed}
        holdRemoteImages={imagesChecking}
        css={body.data?.css}
        inertLinks={inertLinks}
        showQuoted={t("reader.showQuoted")}
        hideQuoted={t("reader.hideQuoted")}
      />
    );
  }

  const truncated = message.text?.truncated === true || body.data?.truncated === true;
  return (
    <MessageCard
      anchor={`message-${message.id}`}
      from={from}
      addressee={addressee}
      side={side}
      avatar={avatar}
      time={time}
      dateTime={message.sent_at}
      // "You → Clara" already names a lone recipient.
      recipients={
        yours && message.to.length + message.cc.length === 1
          ? undefined
          : recipientsLine(t, message, i18n.language)
      }
      rich={message.has_html}
      found={found}
      files={files}
      quoted={
        quoted === undefined ? undefined : (
          <QuotedText showLabel={t("reader.showQuoted")} hideLabel={t("reader.hideQuoted")}>
            {quoted}
          </QuotedText>
        )
      }
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
                {/* In spam they never load: nothing to offer. */}
                {body.data?.images_held === "spam" ? null : (
                  <Button
                    variant="secondary"
                    icon="image"
                    shortcut={shortcuts ? "inbox.show_images" : undefined}
                    onClick={onShowImages}
                  >
                    {t("actions.showImages")}
                  </Button>
                )}
                {trust === undefined ? null : (
                  <Button
                    variant="secondary"
                    shortcut={shortcuts && trustKey ? "inbox.trust_images" : undefined}
                    onClick={trust}
                  >
                    {t("actions.trustImages")}
                  </Button>
                )}
                {body.data?.images_held === undefined || body.data.images_held === null ? null : (
                  <Text tone="tertiary" size="sm">
                    {t(`actions.imagesHeld.${body.data.images_held}`)}
                  </Text>
                )}
              </>
            ) : null}
            {remote > 0 && body.data?.images_trusted === true && !imagesChecking ? (
              <Text tone="tertiary" size="sm">
                {t(sharedInbox ? "actions.imagesTrustedInbox" : "actions.imagesTrusted", {
                  name: sender,
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
