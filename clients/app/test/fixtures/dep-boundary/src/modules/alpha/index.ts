// Allowed: through beta's public surface.
import { betaPublic } from "../beta";
// Forbidden: reaching into beta's internals.
import { betaInternal } from "../beta/internal";

export const alpha = betaPublic + betaInternal;
