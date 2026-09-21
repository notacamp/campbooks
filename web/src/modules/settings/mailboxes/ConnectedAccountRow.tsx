/**
 * ConnectedAccountRow — a single row in the connected mailboxes list.
 * Shows provider, email address, last-sync time, and a Disconnect button
 * (owner only, with an optimistic remove and 4-second undo).
 *
 * Presentation layer — HTML elements and raw copy are intentional here.
 */
/* eslint-disable react/forbid-elements -- ConnectedAccountRow is a design-layer component */
/* eslint-disable react/forbid-component-props -- className on lib/ui icons is intentional */
/* eslint-disable react/jsx-no-literals -- UI copy lives at the surface layer */
import { type FC, useCallback, useEffect, useState } from "react";
import { Loader2, Mail, RefreshCw, X } from "lucide-react";
import { cn } from "~/lib/utils";
import { useDisconnectMutation } from "./hooks";
import type { ConnectedMailbox } from "./types";

// ── Provider labels ───────────────────────────────────────────────────────────

const PROVIDER_LABEL: Record<string, string> = {
  google: "Google",
  zoho: "Zoho",
  microsoft: "Microsoft",
  imap: "IMAP",
};

// ── ConnectedAccountRow ───────────────────────────────────────────────────────

interface ConnectedAccountRowProps {
  mailbox: ConnectedMailbox;
}

export const ConnectedAccountRow: FC<ConnectedAccountRowProps> = ({ mailbox }) => {
  const disconnect = useDisconnectMutation();
  const [undoTimer, setUndoTimer] = useState<ReturnType<typeof setTimeout> | null>(null);
  const [pendingDisconnect, setPendingDisconnect] = useState(false);

  // Clear any pending timer on unmount.
  useEffect(() => () => { if (undoTimer) clearTimeout(undoTimer); }, [undoTimer]);

  const handleDisconnect = useCallback(() => {
    setPendingDisconnect(true);
    const t = setTimeout(() => {
      setPendingDisconnect(false);
      disconnect.mutate(mailbox.id);
    }, 4000);
    setUndoTimer(t);
  }, [mailbox.id, disconnect]);

  const handleUndo = useCallback(() => {
    if (undoTimer) clearTimeout(undoTimer);
    setUndoTimer(null);
    setPendingDisconnect(false);
  }, [undoTimer]);

  const providerLabel = PROVIDER_LABEL[mailbox.provider] ?? mailbox.provider;
  const lastSync = mailbox.last_scanned_at
    ? new Date(mailbox.last_scanned_at).toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      })
    : null;

  return (
    <div
      className={cn(
        "flex items-center gap-3 py-3 px-4",
        "border-b border-line last:border-b-0",
        "transition-opacity duration-[--cb-dur]",
        pendingDisconnect && "opacity-50",
      )}
      data-testid={`mailboxes.connected.${mailbox.id}`}
    >
      {/* Provider icon placeholder */}
      <div
        className="shrink-0 w-8 h-8 rounded-cb-1 bg-surface-2 flex items-center justify-center"
        aria-hidden="true"
      >
        <Mail size={14} className="text-t3" />
      </div>

      {/* Account info */}
      <div className="flex-1 min-w-0">
        <p className="text-[13px] font-medium text-t1 truncate">
          {mailbox.email_address}
        </p>
        <p className="text-[11.5px] text-t3 flex items-center gap-1.5 mt-0.5">
          <span>{providerLabel}</span>
          {mailbox.scanning && (
            <span className="flex items-center gap-1">
              <RefreshCw size={10} className="animate-spin text-t4" aria-hidden="true" />
              <span className="sr-only">Syncing</span>
            </span>
          )}
          {lastSync && <span className="text-t4">· Synced {lastSync}</span>}
        </p>
      </div>

      {/* Actions */}
      {mailbox.is_owner && (
        <div className="shrink-0">
          {pendingDisconnect ? (
            <button
              type="button"
              onClick={handleUndo}
              className={cn(
                "px-2.5 py-1 rounded-cb-1 text-[11.5px] font-medium",
                "bg-surface-2 border border-line text-t2",
                "hover:bg-surface-1 transition-colors duration-[--cb-dur]",
              )}
              data-testid={`mailboxes.connected.${mailbox.id}.undo`}
            >
              Undo
            </button>
          ) : (
            <button
              type="button"
              onClick={handleDisconnect}
              disabled={disconnect.isPending}
              aria-label={`Disconnect ${mailbox.email_address}`}
              className={cn(
                "p-1.5 rounded-cb-1 text-t4",
                "hover:bg-surface-2 hover:text-cb-danger",
                "transition-colors duration-[--cb-dur]",
                "disabled:opacity-50 disabled:cursor-not-allowed",
                "focus-visible:ring-2 focus-visible:ring-cb-accent focus-visible:ring-offset-1 focus-visible:outline-none",
              )}
              data-testid={`mailboxes.connected.${mailbox.id}.disconnect`}
            >
              {disconnect.isPending ? (
                <Loader2 size={14} className="animate-spin" aria-hidden="true" />
              ) : (
                <X size={14} aria-hidden="true" />
              )}
            </button>
          )}
        </div>
      )}
    </div>
  );
};
