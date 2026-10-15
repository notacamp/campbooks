/**
 * EmptyPane — what the reading side of the inbox shows on desktop (≥ lg)
 * when no conversation is open. Keeps the two-pane layout stable so the
 * list never changes width when a message opens or closes.
 */
import { type FC, type ReactNode } from "react";
import { Visor } from "~/lib/ui";

const Key: FC<{ children: ReactNode }> = ({ children }) => (
  <kbd className="inline-flex items-center px-1.5 py-px rounded-cb-1 border border-line bg-surface-1 font-[inherit] text-[11px] text-t3">
    {children}
  </kbd>
);

export const EmptyPane: FC = () => (
  <div
    data-testid="inbox.reading-pane.empty"
    className="flex flex-col items-center justify-center h-full gap-2 p-6 text-center"
  >
    <Visor state="watching" size={28} aria-hidden />
    <p className="text-[14px] font-[550] text-t2">Nothing open</p>
    <p className="text-[12.5px] text-t3">
      Pick a conversation, or move with <Key>j</Key> <Key>k</Key> and close with <Key>Esc</Key>.
    </p>
  </div>
);
