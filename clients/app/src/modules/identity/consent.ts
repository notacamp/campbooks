import { useState } from "react";

import { errorStatus, problemCode } from "~/lib/api";
import type {
  ConsentRefusal,
  ConsentStart,
  UseMailboxConsent,
  WayInFate,
} from "~/modules/connectors";

import { usePasskeys, useStartLink, useWaysIn, type WayIn } from "./api/queries";
import { leavePage, stepUpUrl } from "./model";

/**
 * What disconnecting `address` at `provider` does to your way in, as the
 * server decides it: a way in added only by connecting the mailbox goes with
 * it, unless it is your last way in (provider accounts and passkeys).
 * Whether another of your connections still uses the account is not known
 * here; then the server keeps it, and the page has said it goes.
 */
export function wayInFate(
  items: readonly WayIn[],
  passkeys: number,
  provider: string,
  address: string,
): WayInFate {
  const way = items.find(
    (candidate) =>
      candidate.provider === provider && candidate.email?.toLowerCase() === address.toLowerCase(),
  );
  if (way === undefined) {
    return "none";
  }
  const last = items.length + passkeys <= 1;
  return way.linked_via === "connect" && !last ? "removed" : "kept";
}

/** Why `identity.start_link` refused, in the words the Mailboxes page has. */
function consentRefusal(error: unknown): ConsentRefusal {
  if (problemCode(error) === "step_up_required") {
    return "step_up";
  }
  if (problemCode(error) === "mailbox_unsupported") {
    return "unsupported";
  }
  return errorStatus(error) === 429 ? "rate_limited" : "failed";
}

/**
 * `identity`'s side of the Mailboxes page's front door: the providers a
 * mailbox can be connected from (`identity.list_ways_in`), and
 * `identity.start_link` with `connect` or `reconnect`, coming back to
 * `returnTo`. `leave` navigates the whole page (tests pass a spy).
 */
export function useMailboxConsent(
  { workspaceId, returnTo }: Parameters<UseMailboxConsent>[0],
  leave: (url: string) => void = leavePage,
): ReturnType<UseMailboxConsent> {
  const ways = useWaysIn();
  const passkeys = usePasskeys();
  const link = useStartLink();
  const [refusal, setRefusal] = useState<ConsentRefusal | undefined>();
  const [leaving, setLeaving] = useState(false);

  const start = (request: ConsentStart) => {
    setRefusal(undefined);
    link.mutate(
      request.purpose === "connect"
        ? {
            purpose: "connect",
            provider: request.provider,
            sign_in: request.signIn,
            workspace_id: workspaceId,
            return_to: returnTo,
          }
        : {
            purpose: "reconnect",
            workspace_id: workspaceId,
            connection_id: request.connection,
            return_to: returnTo,
          },
      {
        onSuccess: (url) => {
          setLeaving(true);
          leave(url);
        },
        onError: (failure) => {
          setRefusal(consentRefusal(failure));
        },
      },
    );
  };

  return {
    providers: ways.data?.mailbox_providers,
    providersFailed: ways.isError,
    start,
    starting: link.isPending || leaving,
    refusal,
    signInAgain: () => {
      leave(stepUpUrl(returnTo));
    },
    wayInFate: (provider, address) =>
      ways.data === undefined
        ? undefined
        : wayInFate(ways.data.items, passkeys.data?.items.length ?? 0, provider, address),
  };
}
