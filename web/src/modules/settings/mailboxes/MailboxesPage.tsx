/**
 * MailboxesPage — /settings/mailboxes
 *
 * Shows:
 *   1. Provider cards (Google, Zoho, Microsoft) — each opens the OAuth consent
 *      flow in the same tab; the browser returns via the Rails callback bridge
 *      with ?connected=<provider> (handled by a useEffect below).
 *   2. IMAP connect form (collapsed by default).
 *   3. List of already-connected mailboxes (with per-row disconnect + undo).
 *
 * After an OAuth callback lands back here with ?connected=<provider>, we
 * invalidate the mailboxes query and show a brief success banner.
 * ?error=<reason> shows a brief error banner.
 *
 * Responsive: 1-column on mobile, 2-column grid for provider cards on sm+.
 * No horizontal overflow at 375px.
 *
 * NOTE — Presentation layer exception (mirrors today-view.tsx):
 * This is the design/UI layer for the Mailboxes settings surface.
 * HTML elements and raw copy are intentional; these rules are suppressed so
 * the build stays clean.
 */
/* eslint-disable react/forbid-elements -- MailboxesPage is the design layer; HTML elements are intentional */
/* eslint-disable react/forbid-component-props -- className on lib/ui components is the design layer's prerogative */
/* eslint-disable react/jsx-no-literals -- UI copy lives at the surface layer */
import { type FC, useCallback, useEffect, useState } from "react";
import { useSearch, useNavigate } from "@tanstack/react-router";
import { CheckCircle2, Loader2, RotateCcw, ServerCrash } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { cn } from "~/lib/utils";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import { ConnectedAccountRow } from "./ConnectedAccountRow";
import { ImapForm } from "./ImapForm";
import { ProviderCard } from "./ProviderCard";
import { mailboxKeys, useMailboxesQuery } from "./hooks";

// ── MailboxesPage ─────────────────────────────────────────────────────────────

export const MailboxesPage: FC = () => {
  const queryClient = useQueryClient();
  const navigate = useNavigate();

  // TanStack Router infers search params from the route definition.
  // We cast loosely since the settings route declares them as optional.
  const search = useSearch({ strict: false }) as {
    connected?: string;
    error?: string;
  };

  const { data: mailboxes, isLoading, isError, error, refetch } = useMailboxesQuery();

  const [successBanner, setSuccessBanner] = useState<string | null>(null);
  const [errorBanner, setErrorBanner] = useState<string | null>(null);

  // Handle the OAuth callback return — ?connected=google / ?error=connect_failed
  useEffect(() => {
    if (search.connected) {
      void queryClient.invalidateQueries({ queryKey: mailboxKeys.list() });
      setSuccessBanner(`${capitalize(search.connected)} mailbox connected`);
      void navigate({ to: "/settings/mailboxes", replace: true });
    } else if (search.error) {
      const msg =
        search.error === "cap_reached"
          ? "Mailbox limit reached on your plan"
          : "Could not connect the mailbox — please try again";
      setErrorBanner(msg);
      void navigate({ to: "/settings/mailboxes", replace: true });
    }
  }, [search.connected, search.error, navigate, queryClient]);

  // Dismiss success banner after 6 s, error banner after 8 s.
  useEffect(() => {
    if (!successBanner) return;
    const t = setTimeout(() => setSuccessBanner(null), 6000);
    return () => clearTimeout(t);
  }, [successBanner]);

  useEffect(() => {
    if (!errorBanner) return;
    const t = setTimeout(() => setErrorBanner(null), 8000);
    return () => clearTimeout(t);
  }, [errorBanner]);

  const handleImapSuccess = useCallback(() => {
    setSuccessBanner("IMAP mailbox connected");
  }, []);

  return (
    <div
      className="max-w-2xl w-full mx-auto px-4 sm:px-6 py-6 flex flex-col gap-6"
      data-testid="mailboxes.page"
    >
      {/* Page heading */}
      <div>
        <h1 className="text-[15px] font-semibold text-t1">Mailboxes</h1>
        <p className="text-[12.5px] text-t3 mt-1">
          Connect a mailbox to start reading and sending email through Campbooks.
        </p>
      </div>

      {/* Success banner */}
      {successBanner && (
        <div
          className="flex items-center gap-2 px-3 py-2.5 rounded-cb-2 bg-cb-accent/10 border border-cb-accent/20 text-[12.5px] text-t1"
          role="status"
          aria-live="polite"
          data-testid="mailboxes.banner.success"
        >
          <CheckCircle2 size={14} className="text-cb-accent shrink-0" aria-hidden="true" />
          {successBanner}
        </div>
      )}

      {/* Error banner */}
      {errorBanner && (
        <div
          className="flex items-center gap-2 px-3 py-2.5 rounded-cb-2 bg-cb-danger/10 border border-cb-danger/20 text-[12.5px] text-t1"
          role="alert"
          aria-live="assertive"
          data-testid="mailboxes.banner.error"
        >
          <ServerCrash size={14} className="text-cb-danger shrink-0" aria-hidden="true" />
          {errorBanner}
        </div>
      )}

      {/* Provider cards */}
      <section aria-labelledby="connect-heading">
        <h2
          id="connect-heading"
          className="text-[11px] font-semibold text-t3 uppercase tracking-wider mb-3"
        >
          Connect a mailbox
        </h2>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <ProviderCard provider="google" />
          <ProviderCard provider="zoho" />
          {/* Microsoft shown — returns feature_disabled when ENABLE_MICROSOFT=0 */}
          <ProviderCard provider="microsoft" />
        </div>

        {/* IMAP form */}
        <div className="mt-3">
          <ImapForm onSuccess={handleImapSuccess} />
        </div>
      </section>

      {/* Connected mailboxes */}
      <section aria-labelledby="connected-heading">
        <h2
          id="connected-heading"
          className="text-[11px] font-semibold text-t3 uppercase tracking-wider mb-3"
        >
          Connected
        </h2>

        {isLoading && (
          <div
            className="flex items-center justify-center py-8 text-t4"
            aria-busy="true"
            data-testid="mailboxes.list.loading"
          >
            <Loader2 size={18} className="animate-spin" aria-label="Loading mailboxes" />
          </div>
        )}

        {isError && (
          <div
            className="flex flex-col items-center gap-3 py-8 text-center"
            data-testid="mailboxes.list.error"
          >
            <Visor state="asleep" size={28} label="Error loading mailboxes" />
            <p className="text-[12.5px] text-t3">
              {error instanceof ApiError ? error.message : "Could not load connected mailboxes"}
            </p>
            <button
              type="button"
              onClick={() => refetch()}
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
                "bg-surface-1 border border-line text-[12px] text-t2",
                "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
              )}
            >
              <RotateCcw size={12} aria-hidden="true" />
              Retry
            </button>
          </div>
        )}

        {!isLoading && !isError && (
          <div
            className="rounded-cb-2 bg-surface-1 border border-line overflow-hidden"
            data-testid="mailboxes.list"
          >
            {mailboxes && mailboxes.length > 0 ? (
              mailboxes.map((m) => <ConnectedAccountRow key={m.id} mailbox={m} />)
            ) : (
              <div
                className="flex flex-col items-center gap-2 py-8 text-center"
                data-testid="mailboxes.list.empty"
              >
                <Visor state="asleep" size={24} label="No mailboxes connected" />
                <p className="text-[12.5px] text-t3">
                  No mailboxes connected yet — connect one above.
                </p>
              </div>
            )}
          </div>
        )}
      </section>
    </div>
  );
};

// ── Helpers ───────────────────────────────────────────────────────────────────

const capitalize = (str: string): string =>
  str.charAt(0).toUpperCase() + str.slice(1);
