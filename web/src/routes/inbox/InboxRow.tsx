/**
 * InboxRow — a single row in the inbox list.
 *
 * Shows: Avatar | sender name + relative time | subtitle | annotation | unread dot
 *
 * The row is keyboard-navigable: it accepts focus and responds to
 * Enter (open), A/Delete (archive).
 */
import { type FC, type KeyboardEvent, useCallback } from "react";
import { Archive } from "lucide-react";
import { Avatar, Signal } from "~/lib/ui";
import type { InboxRow as InboxRowData } from "~/modules/inbox/types";
import { annotationFor, signalPlaceFor } from "~/modules/inbox/hooks";
import { formatRelativeTime } from "./utils";
import { cn } from "~/lib/utils";

interface InboxRowProps {
  row: InboxRowData;
  selected: boolean;
  onSelect: (id: number) => void;
  onArchive: (id: number) => void;
}

export const InboxRow: FC<InboxRowProps> = ({ row, selected, onSelect, onArchive }) => {
  const place = signalPlaceFor(row.verb);
  const annotation = annotationFor(row);
  const avatarKind = row.counterpart_type === "Organization" ? "service" : "person";
  const relTime = row.last_activity_at ? formatRelativeTime(row.last_activity_at) : "";

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLButtonElement>): void => {
      if (e.key === "a" || e.key === "e") {
        e.preventDefault();
        onArchive(row.id);
      }
    },
    [row.id, onArchive],
  );

  return (
    <div
      data-testid={`inbox.list.row.${row.id}`}
      className={cn(
        "group relative flex items-start gap-3 px-4 py-3 cursor-pointer",
        "border-b border-line transition-colors duration-[--cb-dur]",
        "focus-visible:outline-2 focus-visible:outline-accent-cb focus-visible:outline-offset-[-2px]",
        selected ? "bg-surface-1" : "hover:bg-surface-1",
      )}
      role="option"
      aria-selected={selected}
      tabIndex={0}
      onClick={() => onSelect(row.id)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(row.id);
        }
        if (e.key === "a" || e.key === "e") {
          e.preventDefault();
          onArchive(row.id);
        }
      }}
      aria-label={`${row.name}${row.unread ? ", unread" : ""}`}
    >
      {/* Avatar */}
      <Avatar
        initials={row.avatar_initial ?? undefined}
        email={row.avatar_email ?? undefined}
        kind={avatarKind}
        place={place}
        size={32}
        className="mt-0.5 shrink-0"
      />

      {/* Content */}
      <div className="flex-1 min-w-0">
        {/* Row 1: name + time */}
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "flex-1 truncate text-[13px]",
              row.unread ? "font-[600] text-t1" : "font-[450] text-t2",
            )}
          >
            {row.name}
          </span>
          {relTime && (
            <time
              dateTime={row.last_activity_at ?? undefined}
              className="text-[11px] text-t4 tabular-nums shrink-0"
            >
              {relTime}
            </time>
          )}
          {/* Unread dot */}
          {row.unread && (
            <span
              className="w-[6px] h-[6px] rounded-full bg-accent-cb shrink-0"
              aria-hidden="true"
            />
          )}
        </div>

        {/* Row 2: subtitle */}
        {row.subtitle && (
          <p className="text-[12px] text-t3 truncate mt-0.5">{row.subtitle}</p>
        )}

        {/* Row 3: annotation */}
        {annotation && (
          <div className="flex items-center gap-1.5 mt-1">
            <Signal.Dot place={place} />
            <span className="text-[11.5px] text-t3 truncate">{annotation}</span>
          </div>
        )}
      </div>

      {/* Archive button — visible on hover/focus */}
      <button
        type="button"
        data-testid={`inbox.list.archive.${row.id}`}
        onClick={(e) => {
          e.stopPropagation();
          onArchive(row.id);
        }}
        onKeyDown={handleKeyDown}
        aria-label={`Archive ${row.name}`}
        className={cn(
          "shrink-0 mt-0.5 w-7 h-7 flex items-center justify-center rounded-cb-1",
          "text-t4 opacity-0 group-hover:opacity-100 focus:opacity-100",
          "hover:text-t2 hover:bg-surface-2 transition-all duration-[--cb-dur]",
        )}
      >
        <Archive size={13} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </div>
  );
};
