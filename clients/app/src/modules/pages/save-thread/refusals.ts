import { errorStatus, problemCode } from "~/lib/api";

/** The most a file may weigh to be saved to a page: 20 MiB (`pages.attach_from_message`). */
export const FILE_MAX_BYTES = 20 * 1024 * 1024;

/** Why saving attachments was refused, said by name. */
export type Refusal =
  /** This sign-in may not read mail (403 `insufficient_scope`). */
  | { reason: "scope" }
  /** The thread changed, or a file is gone (404). */
  | { reason: "changed" }
  /** A file is over the cap (413 `file_too_large`); `name` when the file is known. */
  | { reason: "tooLarge"; name?: string | undefined }
  /** Anything else. */
  | { reason: "failed" };

/** A file to include, as the dialog lists it. */
export interface Includable {
  name: string;
  size: number;
}

/** The first file over the cap, so the dialog can say which before anything is sent. */
export function tooLarge(files: readonly Includable[]): Includable | undefined {
  return files.find((file) => file.size > FILE_MAX_BYTES);
}

/**
 * Why `pages.attach_from_message` refused, from the API's answer. `files` is
 * what was asked for, to name the file when only one could be meant.
 */
export function refusalOf(error: unknown, files: readonly Includable[]): Refusal {
  const status = errorStatus(error);
  if (status === 403 && problemCode(error) === "insufficient_scope") {
    return { reason: "scope" };
  }
  if (status === 404) {
    return { reason: "changed" };
  }
  if (status === 413) {
    const over = tooLarge(files) ?? (files.length === 1 ? files[0] : undefined);
    return { reason: "tooLarge", name: over?.name };
  }
  return { reason: "failed" };
}
