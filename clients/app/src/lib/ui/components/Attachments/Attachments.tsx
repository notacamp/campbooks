import { useTranslation } from "react-i18next";

import type { ShortcutId } from "~/lib/shortcuts";

import { formatSize } from "../../internal/format-size";
import { Overlay } from "../../internal/Overlay";
import { Icon, type IconName } from "../Icon/Icon";
import { IconButton } from "../IconButton/IconButton";
import { Kbd, KbdHint } from "../Kbd/Kbd";

import styles from "./Attachments.module.css";

/** What a file can be shown as: images and PDFs have a preview. */
export type AttachmentKind = "image" | "pdf" | "file";

export interface AttachmentListItem {
  id: string;
  /** Its file name, already cleaned of control and bidirectional characters. */
  name: string;
  /** In bytes. */
  size: number;
  kind: AttachmentKind;
}

export interface AttachmentListProps {
  attachments: readonly AttachmentListItem[];
  onDownload: (id: string) => void;
  onPreview: (id: string) => void;
  /** The attachment being fetched, if any. */
  busy?: string | undefined;
  /** A line under the list when something went wrong. */
  error?: string | undefined;
  /** The shortcut that moves through attachments (its hint is shown). */
  shortcut?: ShortcutId | undefined;
}

const ICONS: Record<AttachmentKind, IconName> = {
  image: "image",
  pdf: "document",
  file: "attachment",
};

/** Each attachment's main control, for {@link focusNextAttachment}. */
const OPEN_SELECTOR = "[data-attachment-open]";

/**
 * Move focus to the next attachment's main control in `root` (after the one
 * focused, wrapping around). Returns whether there was one.
 */
export function focusNextAttachment(root: ParentNode = document): boolean {
  const controls = [...root.querySelectorAll<HTMLButtonElement>(OPEN_SELECTOR)];
  if (controls.length === 0) {
    return false;
  }
  const current = controls.indexOf(document.activeElement as HTMLButtonElement);
  const next = controls[(current + 1) % controls.length];
  // Focusing scrolls it into view.
  next?.focus();
  return true;
}

/**
 * A message's attachments: type icon, name, size, Preview (images and PDFs)
 * and Download. The name opens it: its preview when it has one, else the
 * download.
 */
export function AttachmentList({
  attachments,
  onDownload,
  onPreview,
  busy,
  error,
  shortcut,
}: AttachmentListProps) {
  const { t, i18n } = useTranslation("ui");
  if (attachments.length === 0) {
    return null;
  }
  return (
    <section className={styles["section"]} aria-label={t("attachments.label")}>
      <div className={styles["head"]}>
        <span className={styles["count"]}>
          {t("attachments.count", { count: attachments.length })}
        </span>
        {shortcut === undefined ? null : <KbdHint shortcut={shortcut} />}
      </div>
      <ul className={styles["list"]}>
        {attachments.map((attachment) => {
          const previewable = attachment.kind !== "file";
          const open = () => {
            if (previewable) {
              onPreview(attachment.id);
            } else {
              onDownload(attachment.id);
            }
          };
          return (
            <li
              key={attachment.id}
              className={styles["item"]}
              aria-busy={busy === attachment.id || undefined}
            >
              <Icon name={ICONS[attachment.kind]} />
              <button
                type="button"
                className={styles["name"]}
                title={attachment.name}
                onClick={open}
                data-attachment-open=""
              >
                <span className={styles["nameText"]}>{attachment.name}</span>
              </button>
              <span className={styles["size"]}>
                {busy === attachment.id ? t("loading") : formatSize(attachment.size, i18n.language)}
              </span>
              {previewable ? (
                <IconButton
                  icon="preview"
                  label={t("attachments.previewOf", { name: attachment.name })}
                  onClick={() => {
                    onPreview(attachment.id);
                  }}
                />
              ) : null}
              <IconButton
                icon="download"
                label={t("attachments.downloadOf", { name: attachment.name })}
                onClick={() => {
                  onDownload(attachment.id);
                }}
              />
            </li>
          );
        })}
      </ul>
      {error === undefined ? null : (
        <p className={styles["error"]} role="alert">
          {error}
        </p>
      )}
    </section>
  );
}

export interface ImagePreviewProps {
  /** The picture to show (a `blob:` URL); `undefined` while it loads. */
  src: string | undefined;
  /** Its file name. */
  name: string;
  onClose: () => void;
  onDownload: () => void;
}

/**
 * An attached picture, in a dialog: shown only as an image (never as a
 * page), with its name, Download and Close (Esc).
 */
export function ImagePreview({ src, name, onClose, onDownload }: ImagePreviewProps) {
  const { t } = useTranslation("ui");
  return (
    <Overlay open onClose={onClose} label={t("attachments.previewTitle", { name })}>
      <div className={styles["previewHead"]}>
        <Icon name="image" />
        <h2 className={styles["previewName"]} title={name}>
          {name}
        </h2>
        <IconButton
          icon="download"
          label={t("attachments.downloadOf", { name })}
          onClick={onDownload}
        />
        <Kbd>Esc</Kbd>
        <IconButton icon="close" label={t("close")} onClick={onClose} />
      </div>
      <div className={styles["stage"]} aria-busy={src === undefined || undefined}>
        {src === undefined ? (
          <span className={styles["loading"]}>{t("loading")}</span>
        ) : (
          <img className={styles["image"]} src={src} alt={name} />
        )}
      </div>
    </Overlay>
  );
}
