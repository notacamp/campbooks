/**
 * OAuthSignInButton — a single "Sign in with <Provider>" button.
 *
 * On click: lazily fetches GET /api/app/oauth/sign_in_url?provider=&return_to=
 * then immediately navigates the browser tab to the returned authorize_url.
 * Shows a spinner while fetching (typically < 200 ms).
 *
 * Design-layer component — raw HTML and Tailwind tokens are intentional here.
 */
import { type FC, useCallback, useState } from "react";
import { Loader2 } from "lucide-react";
import { cn } from "~/lib/utils";
import { useOAuthSignInUrlQuery } from "~/lib/api";
import type { OAuthProvider } from "~/lib/api";
import { GoogleMark, ZohoMark, MicrosoftMark } from "./OAuthProviderMarks";

// ── Provider metadata ─────────────────────────────────────────────────────────

const PROVIDER_META: Record<OAuthProvider, { label: string }> = {
  google: { label: "Google" },
  zoho: { label: "Zoho" },
  microsoft: { label: "Microsoft" },
};

const MARKS: Record<OAuthProvider, FC> = {
  google: GoogleMark,
  zoho: ZohoMark,
  microsoft: MicrosoftMark,
};

// ── OAuthSignInButton ─────────────────────────────────────────────────────────

interface OAuthSignInButtonProps {
  provider: OAuthProvider;
}

export const OAuthSignInButton: FC<OAuthSignInButtonProps> = ({ provider }) => {
  const meta = PROVIDER_META[provider];
  const Mark = MARKS[provider];

  // Enabled only after the user clicks — avoids a fetch on every page load.
  const [enabled, setEnabled] = useState(false);

  const { data, isFetching, error } = useOAuthSignInUrlQuery(provider, {
    enabled,
  });

  // As soon as the URL arrives, navigate immediately.
  if (data?.authorize_url && enabled) {
    window.location.href = data.authorize_url;
  }

  const handleClick = useCallback(() => {
    if (data?.authorize_url) {
      window.location.href = data.authorize_url;
      return;
    }
    setEnabled(true);
  }, [data]);

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={isFetching}
      aria-busy={isFetching}
      aria-label={`Sign in with ${meta.label}`}
      data-testid={`login.oauth.${provider}`}
      className={cn(
        "flex items-center gap-2.5 w-full h-9 rounded-cb-1 px-3",
        "border border-line bg-surface-1 text-t1",
        "text-[13px] font-[480]",
        "hover:bg-ground transition-colors duration-[--cb-dur]",
        "disabled:opacity-50 disabled:cursor-not-allowed",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[--ring]",
      )}
    >
      {isFetching ? (
        <Loader2 size={16} className="shrink-0 animate-spin text-t3" />
      ) : (
        <span className="shrink-0 flex items-center">
          <Mark />
        </span>
      )}
      <span>{"Sign in with "}{meta.label}</span>
      {error != null && (
        <span
          className="ml-auto text-[11px] text-[--destructive]"
          role="alert"
        >
          {"failed — retry"}
        </span>
      )}
    </button>
  );
};
