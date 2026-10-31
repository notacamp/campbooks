/**
 * What a draft's attachments may be, mirrored from the server
 * (`conversations`' `domain/attachments.rs` and `domain/drafts.rs`; a test
 * reads the Rust list). The server checks every upload again: these rules
 * only spare the member a wait for an answer they can know in advance.
 */

/** Most bytes of attachments a message may carry (Gmail's limit). */
export const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024;

/** Most files a draft may carry. */
export const MAX_ATTACHMENTS = 20;

/** Extensions mail providers refuse to carry, lower-case. */
export const BLOCKED_EXTENSIONS: readonly string[] = [
  "ade",
  "adp",
  "apk",
  "appx",
  "appxbundle",
  "bat",
  "cab",
  "chm",
  "cmd",
  "com",
  "cpl",
  "diagcab",
  "diagcfg",
  "diagpack",
  "dll",
  "dmg",
  "ex",
  "ex_",
  "exe",
  "hta",
  "img",
  "ins",
  "iso",
  "isp",
  "jar",
  "jnlp",
  "js",
  "jse",
  "lib",
  "lnk",
  "mde",
  "msc",
  "msi",
  "msix",
  "msixbundle",
  "msp",
  "mst",
  "nsh",
  "pif",
  "ps1",
  "scr",
  "sct",
  "shb",
  "sys",
  "vb",
  "vbe",
  "vbs",
  "vhd",
  "vxd",
  "wsc",
  "wsf",
  "wsh",
  "xll",
];

/** Why a file is not attached: the keys of `compose.attach.refused`. */
export type RefusalReason =
  | "file_empty"
  | "filename_missing"
  | "attachment_blocked"
  | "attachments_too_large"
  | "attachments_too_large_for_mailbox"
  | "too_many_attachments"
  | "upload_incomplete"
  | "request_timed_out";

export interface Refusal {
  file: File;
  reason: RefusalReason;
}

const REASONS = new Set<string>([
  "file_empty",
  "filename_missing",
  "attachment_blocked",
  "attachments_too_large",
  "attachments_too_large_for_mailbox",
  "too_many_attachments",
  "upload_incomplete",
  "request_timed_out",
]);

/** The refusal a server problem code stands for, if it is one. */
export function refusalOfCode(code: string): RefusalReason | undefined {
  return REASONS.has(code) ? (code as RefusalReason) : undefined;
}

/** A file name's extension as the server reads it: after the last dot, trimmed, lower-case. */
export function extensionOf(name: string): string {
  const base = name.split(/[/\\]/u).at(-1) ?? "";
  const trimmed = base.replaceAll(/^[.\s]+|[.\s]+$/gu, "");
  const dot = trimmed.lastIndexOf(".");
  return dot === -1 ? "" : trimmed.slice(dot + 1).toLowerCase();
}

/** Whether mail providers block a file of this name. */
export function isBlocked(name: string): boolean {
  return BLOCKED_EXTENSIONS.includes(extensionOf(name));
}

/** What the draft already holds: bytes and files, uploads on their way included. */
export interface Held {
  bytes: number;
  count: number;
}

/**
 * Split `files` into the ones to upload and the ones refused up front: an
 * empty file, a blocked type, one that would take the total past `budget`
 * (25 MiB, or less for a mailbox that carries less) or the draft past 20
 * files (counting what is held and what is accepted before it in the same
 * drop).
 */
export function screen(
  files: readonly File[],
  held: Held,
  budget: number = MAX_ATTACHMENT_BYTES,
): { accepted: File[]; refused: Refusal[] } {
  const accepted: File[] = [];
  const refused: Refusal[] = [];
  let bytes = held.bytes;
  let count = held.count;
  for (const file of files) {
    let reason: RefusalReason | undefined;
    if (file.size === 0) {
      reason = "file_empty";
    } else if (isBlocked(file.name)) {
      reason = "attachment_blocked";
    } else if (count >= MAX_ATTACHMENTS) {
      reason = "too_many_attachments";
    } else if (bytes + file.size > budget) {
      reason =
        budget < MAX_ATTACHMENT_BYTES
          ? "attachments_too_large_for_mailbox"
          : "attachments_too_large";
    }
    if (reason === undefined) {
      accepted.push(file);
      bytes += file.size;
      count += 1;
    } else {
      refused.push({ file, reason });
    }
  }
  return { accepted, refused };
}

/** `bytes` in MB as the server says it: one decimal under 10 MB ("2.1"), whole MiB above. */
export function megabytes(bytes: number): string {
  return bytes < 10_000_000
    ? (Math.floor(bytes / 100_000) / 10).toFixed(1)
    : String(Math.floor(bytes / (1024 * 1024)));
}
