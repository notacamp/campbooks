/**
 * ReadingPane — message detail view shown alongside the list at ≥ 900px.
 *
 * Pilot scope (stubbed):
 *   - Sender header: Avatar + name
 *   - Scout's "what this is": stand_line
 *   - Thread body: stubbed placeholder
 *   - Actions: Use Scout's draft (stub) / Reply (stub) / Archive
 *
 * Below 900px the pane is hidden; the row opens a sheet/modal in a later step.
 */
import { type FC } from "react";
import { X, Send, Reply, Archive } from "lucide-react";
import { Avatar, Visor, Signal } from "~/lib/ui";
import type { InboxRow } from "~/modules/inbox/types";
import { annotationFor, signalPlaceFor } from "~/modules/inbox/hooks";
import { cn } from "~/lib/utils";

interface ReadingPaneProps {
  row: InboxRow;
  onClose: () => void;
  onArchive: (id: number) => void;
}

export const ReadingPane: FC<ReadingPaneProps> = ({ row, onClose, onArchive }) => {
  const place = signalPlaceFor(row.verb);
  const annotation = annotationFor(row);
  const avatarKind = row.counterpart_type === "Organization" ? "service" : "person";

  return (
    <div
      data-testid="inbox.reading-pane.root"
      className={cn(
        "flex flex-col h-full border-l border-line bg-ground",
        "overflow-hidden",
      )}
    >
      {/* Header */}
      <div className="flex items-center gap-3 px-4 py-3 border-b border-line">
        <Avatar
          initials={row.avatar_initial ?? undefined}
          email={row.avatar_email ?? undefined}
          kind={avatarKind}
          place={place}
          size={30}
        />
        <div className="flex-1 min-w-0">
          <p className="text-[13px] font-[550] text-t1 truncate">{row.name}</p>
          {row.avatar_email && (
            <p className="text-[11px] text-t3 truncate">{row.avatar_email}</p>
          )}
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close reading pane"
          className={cn(
            "shrink-0 w-7 h-7 flex items-center justify-center rounded-cb-1",
            "text-t4 hover:text-t2 hover:bg-surface-1",
            "transition-colors duration-[--cb-dur]",
          )}
        >
          <X size={14} strokeWidth={2} aria-hidden="true" />
        </button>
      </div>

      {/* Scout's what-this-is */}
      {(row.stand_line || annotation) && (
        <div
          className={cn(
            "mx-4 mt-3 px-3 py-2.5 rounded-cb-2",
            "bg-surface-1 cb-raised",
          )}
        >
          <div className="flex items-center gap-1.5 mb-1">
            <Visor size={14} state="found" aria-hidden />
            <span className="text-[10.5px] font-[650] uppercase tracking-wide text-t4">
              Scout
            </span>
          </div>
          <div className="flex items-start gap-1.5">
            <Signal.Dot place={place} className="mt-1" />
            <p className="text-[13px] text-t2 leading-relaxed">
              {row.stand_line ?? annotation}
            </p>
          </div>
        </div>
      )}

      {/* Thread body — stubbed */}
      <div className="flex-1 overflow-y-auto px-4 py-4">
        <div
          data-testid="inbox.reading-pane.body"
          className={cn(
            "rounded-cb-2 bg-surface-1 cb-raised p-4",
            "text-[13px] text-t3 italic",
          )}
        >
          Thread body — coming soon
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2 px-4 py-3 border-t border-line">
        <button
          type="button"
          aria-label="Use Scout's draft"
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
            "bg-surface-1 border border-line text-[12.5px] text-t2 font-[500]",
            "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
          )}
        >
          <Visor size={13} state="watching" aria-hidden />
          Scout&apos;s draft
        </button>
        <button
          type="button"
          aria-label="Reply (coming soon)"
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
            "bg-surface-1 border border-line text-[12.5px] text-t2 font-[500]",
            "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
          )}
        >
          <Reply size={13} strokeWidth={1.8} aria-hidden="true" />
          Reply
        </button>
        <button
          type="button"
          data-testid="inbox.reading-pane.archive"
          onClick={() => onArchive(row.id)}
          aria-label="Archive"
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1 ml-auto",
            "text-[12.5px] text-t3 font-[500]",
            "hover:text-t2 hover:bg-surface-1 transition-colors duration-[--cb-dur]",
          )}
        >
          <Archive size={13} strokeWidth={1.8} aria-hidden="true" />
          Archive
        </button>

        {/* Send icon as visual hint for future compose */}
        <span aria-hidden="true" className="text-t4 opacity-30">
          <Send size={12} strokeWidth={1.8} />
        </span>
      </div>
    </div>
  );
};
