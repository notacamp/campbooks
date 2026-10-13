import { useCallback, useEffect, useRef, useState } from "react";

import { problemCode } from "~/lib/api";

import { signInWithPasskey } from "../../api/queries";
import { autofillSupported, ceremonyFailure, getPasskey, passkeysSupported } from "../../webauthn";

/** Why a passkey sign-in didn't go through, as the screen explains it. */
type PasskeyProblem =
  "cancelled" | "not_accepted" | "suspended" | "expired" | "rate_limited" | "failed";

export interface PasskeySignIn {
  /** Whether this browser can use passkeys at all. */
  supported: boolean;
  /** Whether a field offers this site's passkeys in its autofill. */
  autofill: boolean;
  /** A dialog sign-in is under way. */
  busy: boolean;
  /** Why the last attempt failed. */
  problem: PasskeyProblem | undefined;
  /** Sign in in the browser's passkey dialog (the button, `P`). */
  signIn: () => void;
}

function problemOf(error: unknown): PasskeyProblem {
  switch (problemCode(error)) {
    case "passkey_not_accepted":
      return "not_accepted";
    case "account_suspended":
      return "suspended";
    case "passkey_expired":
      return "expired";
    case "rate_limited":
      return "rate_limited";
    case undefined:
      return ceremonyFailure(error) === "cancelled" ? "cancelled" : "failed";
    default:
      return "failed";
  }
}

/**
 * Passkey sign-in for the waiting request: autofill (conditional UI) waits in
 * the background while the page is open, and `signIn` opens the browser's
 * dialog instead. Either way, a passkey that verifies leaves for the client
 * with a code. After a failure that came from the person (a dialog, or a
 * passkey picked from autofill and refused), autofill is offered again.
 */
export function usePasskeySignIn(
  request: string,
  enabled: boolean,
  leave: (url: string) => void,
): PasskeySignIn {
  const supported = enabled && passkeysSupported();
  const [autofill, setAutofill] = useState(false);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<PasskeyProblem | undefined>();
  const waiting = useRef<AbortController | null>(null);
  const autofillOn = useRef(false);

  const attempt = useCallback(
    async function run(dialog: boolean): Promise<void> {
      waiting.current?.abort();
      const controller = new AbortController();
      waiting.current = controller;
      if (dialog) {
        setBusy(true);
        setProblem(undefined);
      }
      try {
        const redirect = await signInWithPasskey({ request }, (options) =>
          getPasskey(options, { autofill: !dialog, signal: controller.signal }),
        );
        leave(redirect);
      } catch (error) {
        if (controller.signal.aborted) {
          return;
        }
        const failed = problemOf(error);
        // Dismissing the autofill suggestions is no failure worth a word.
        if (dialog || failed !== "cancelled") {
          setProblem(failed);
        }
        if (dialog) {
          setBusy(false);
        }
        const refused = failed === "not_accepted" || failed === "expired";
        if (autofillOn.current && (dialog || refused)) {
          await run(false);
        }
      }
    },
    [request, leave],
  );

  useEffect(() => {
    if (!supported) {
      return;
    }
    let live = true;
    void autofillSupported().then((available) => {
      if (live && available) {
        autofillOn.current = true;
        setAutofill(true);
        void attempt(false);
      }
    });
    return () => {
      live = false;
      autofillOn.current = false;
      waiting.current?.abort();
    };
  }, [supported, attempt]);

  const signIn = useCallback(() => {
    void attempt(true);
  }, [attempt]);

  return { supported, autofill, busy, problem, signIn };
}
