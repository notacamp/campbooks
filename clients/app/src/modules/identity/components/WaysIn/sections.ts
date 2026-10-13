import type { ComponentType } from "react";

/**
 * The sections other ways in add to Settings → Ways to sign in, below the
 * provider accounts, in order. Each fetches its own data and takes no props.
 * Passkeys (slice S3) add `PasskeysSection` here.
 */
export const WAY_IN_SECTIONS: readonly ComponentType[] = [];
