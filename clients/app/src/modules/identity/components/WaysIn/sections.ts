import type { ComponentType } from "react";

import { PasskeysSection } from "../Passkeys/PasskeysSection";

/**
 * The sections other ways in add to Settings → Ways to sign in, below the
 * provider accounts, in order. Each fetches its own data and takes no props.
 */
export const WAY_IN_SECTIONS: readonly ComponentType[] = [PasskeysSection];
