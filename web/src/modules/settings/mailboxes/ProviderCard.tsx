/**
 * ProviderCard — a card for a connectable OAuth provider (Google, Zoho,
 * Microsoft). Shows the provider name + description and a "Connect" button.
 * On click, fetches the authorize URL and navigates the browser tab to it.
 *
 * Presentation layer — HTML elements and raw copy are intentional here.
 */
/* eslint-disable react/forbid-elements -- ProviderCard is a design-layer component */
/* eslint-disable react/forbid-component-props -- className on lib/ui icons is intentional */
/* eslint-disable react/jsx-no-literals -- UI copy lives at the surface layer */
import { type FC, useCallback, useState } from "react";
import { ExternalLink, Loader2 } from "lucide-react";
import { cn } from "~/lib/utils";
import { useAuthorizeUrlQuery } from "./hooks";
import type { MailboxProvider } from "./types";

// ── Provider metadata ─────────────────────────────────────────────────────────

const PROVIDER_META: Record<
  Exclude<MailboxProvider, "imap">,
  { label: string; description: string }
> = {
  google: {
    label: "Google",
    description: "Gmail · Google Calendar",
  },
  zoho: {
    label: "Zoho",
    description: "Zoho Mail · Zoho Calendar",
  },
  microsoft: {
    label: "Microsoft 365",
    description: "Outlook / Exchange (work accounts)",
  },
};

// ── ProviderCard ──────────────────────────────────────────────────────────────

interface ProviderCardProps {
  provider: Exclude<MailboxProvider, "imap">;
  disabled?: boolean;
}

export const ProviderCard: FC<ProviderCardProps> = ({ provider, disabled = false }) => {
  const meta = PROVIDER_META[provider];
  const [enabled, setEnabled] = useState(false);

  const { data, isFetching, error } = useAuthorizeUrlQuery(provider, { enabled });

  const handleConnect = useCallback(() => {
    if (data?.authorize_url) {
      window.location.href = data.authorize_url;
      return;
    }
    setEnabled(true);
  }, [data]);

  // As soon as the URL is fetched while pending, navigate immediately.
  if (data?.authorize_url && enabled) {
    window.location.href = data.authorize_url;
  }

  return (
    <div
      className={cn(
        "flex items-center justify-between gap-4",
        "p-4 rounded-cb-2 bg-surface-1 border border-line",
        "transition-colors duration-[--cb-dur]",
        disabled && "opacity-40 pointer-events-none",
      )}
      data-testid={`mailboxes.provider.${provider}`}
    >
      <div className="min-w-0">
        <p className="text-[13.5px] font-medium text-t1 leading-tight">{meta.label}</p>
        <p className="text-[12px] text-t3 mt-0.5">{meta.description}</p>
        {error && (
          <p className="text-[11px] text-cb-danger mt-1" role="alert">
            Could not load connect URL — try again
          </p>
        )}
      </div>

      <button
        type="button"
        onClick={handleConnect}
        disabled={isFetching || disabled}
        aria-busy={isFetching}
        aria-label={`Connect ${meta.label}`}
        className={cn(
          "flex items-center gap-1.5 shrink-0",
          "px-3 py-1.5 rounded-cb-1 text-[12.5px] font-medium",
          "bg-cb-accent text-white",
          "hover:bg-cb-accent/90 active:scale-[0.97]",
          "transition-all duration-[--cb-dur]",
          "disabled:opacity-60 disabled:cursor-not-allowed",
          "focus-visible:ring-2 focus-visible:ring-cb-accent focus-visible:ring-offset-2 focus-visible:outline-none",
        )}
      >
        {isFetching ? (
          <Loader2 size={12} className="animate-spin" aria-hidden="true" />
        ) : (
          <ExternalLink size={12} aria-hidden="true" />
        )}
        Connect
      </button>
    </div>
  );
};
