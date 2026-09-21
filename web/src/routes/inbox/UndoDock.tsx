/**
 * UndoDock — in-place confirmation of an archive action with Undo.
 *
 * Shows "Archived · <name> · Undo" at the bottom of the list pane.
 * Clicking Undo calls the unarchive mutation which restores the row
 * after the server round-trip.
 *
 * Auto-dismisses after 6 seconds if not acted on.
 */
import { type FC, useEffect, useCallback } from "react";
import { cn } from "~/lib/utils";

interface UndoDockProps {
  archivedName: string;
  onUndo: () => void;
  onDismiss: () => void;
}

const AUTO_DISMISS_MS = 6_000;

export const UndoDock: FC<UndoDockProps> = ({ archivedName, onUndo, onDismiss }) => {
  // Auto-dismiss after timeout
  useEffect(() => {
    const timer = setTimeout(onDismiss, AUTO_DISMISS_MS);
    return () => clearTimeout(timer);
  }, [onDismiss]);

  const handleUndo = useCallback((): void => {
    onUndo();
    onDismiss();
  }, [onUndo, onDismiss]);

  return (
    <div
      data-testid="inbox.undodock.root"
      role="status"
      aria-live="polite"
      className={cn(
        "flex items-center justify-between gap-3 px-4 py-2.5",
        "border-t border-line bg-surface-1",
        "text-[12.5px] text-t2",
      )}
    >
      <span className="truncate">
        Archived
        {archivedName ? ` · ${archivedName}` : ""}
      </span>
      <button
        type="button"
        data-testid="inbox.undodock.undo"
        onClick={handleUndo}
        className={cn(
          "shrink-0 text-accent-cb font-[550] text-[12.5px]",
          "hover:opacity-80 transition-opacity duration-[--cb-dur]",
        )}
      >
        Undo
      </button>
    </div>
  );
};
