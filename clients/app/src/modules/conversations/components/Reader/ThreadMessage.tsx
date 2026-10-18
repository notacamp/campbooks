import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

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
import { useMessageBody, type MessageView } from "../../api/queries";
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
   * (campbooks-cloud#200); absent where the member may not teach rules.
   */
  onTrustImages?: (() => void) | undefined;
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
  const sender = displayName(message.from);
  const yours = message.direction === "outbound";
  const from = yours ? t("reader.you") : sender;
  const side = yours ? "yours" : "theirs";
  const addressee = yours ? addresseeLine(t, message) : undefined;
  const avatar = yours ? { kind, initials: initialsOf(sender) } : { kind };
  const time = messageTime(message.sent_at, i18n.language, now);
  const text = message.text?.content ?? "";
  const files = useAttachments(workspaceId, conversationId, message, t, shortcuts);

  if (!open) {
    return (
      <FoldedMessage
        from={from}
        addressee={addressee}
        side={side}
        avatar={avatar}
        time={time}
        dateTime={message.sent_at}
        snippet={text.replaceAll(/\s+/gu, " ").trim()}
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
        remoteImages={body.data?.images_shown ?? false}
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
                <Button
                  variant="secondary"
                  icon="image"
                  shortcut={shortcuts ? "inbox.show_images" : undefined}
                  onClick={onShowImages}
                >
                  {t("actions.showImages")}
                </Button>
                {onTrustImages === undefined ? null : (
                  <Button
                    variant="secondary"
                    shortcut={shortcuts ? "inbox.trust_images" : undefined}
                    onClick={onTrustImages}
                  >
                    {t("actions.trustImages", { name: sender })}
                  </Button>
                )}
              </>
            ) : null}
            {remote > 0 && body.data?.images_trusted === true ? (
              <Text tone="tertiary" size="sm">
                {t("actions.imagesTrusted", { name: sender })}
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
