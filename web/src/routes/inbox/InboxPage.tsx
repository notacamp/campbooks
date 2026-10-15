/**
 * InboxPage — the unified inbox surface at /inbox.
 *
 * Features:
 *   - Search bar
 *   - Important / Everything tabs (Important = needs_you == true)
 *   - Infinite-paged list of InboxRows (Avatar, name, time, subject, annotation, unread)
 *   - Archive with in-place Undo (optimistic via TanStack Query)
 *   - Two panes from lg (1024px): a fixed 400px list + the reading pane (or
 *     EmptyPane), so the list never stretches or changes width. Below lg one
 *     pane at a time: tapping a row replaces the list with the message.
 *   - The open conversation can be held in the URL (?open=<id>) by the route
 *     (see InboxRoute) so browser/phone Back closes it.
 *   - "Skim the long tail" stub entry
 *   - Loading skeletons, empty state, error + retry
 *   - Keyboard: j/k navigate, a/e archive, Enter open pane, Esc close
 *
 * Realtime: the UserSyncChannel is wired via useInboxRealtimeSync.
 */
import {
  type FC,
  useState,
  useCallback,
  useRef,
  useEffect,
  useMemo,
} from "react";
import { Search, Layers, RotateCcw } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";
import { Visor } from "~/lib/ui";
import {
  useInboxInfiniteQuery,
  useArchiveMutation,
  useInboxRealtimeSync,
} from "~/modules/inbox/hooks";
import type { InboxRow, InboxTab } from "~/modules/inbox/types";
import { TAB_LABELS } from "~/modules/inbox/types";
import { InboxRow as InboxRowComponent } from "./InboxRow";
import { SkeletonRow } from "./SkeletonRow";
import { ReadingPane } from "./ReadingPane";
import { EmptyPane } from "./EmptyPane";
import { UndoDock } from "./UndoDock";
import { cn } from "~/lib/utils";

const SKELETON_COUNT = 8;

// ── UndoState ─────────────────────────────────────────────────────────────────

interface UndoState {
  archivedId: number;
  archivedName: string;
}

// ── InboxPage ─────────────────────────────────────────────────────────────────

interface InboxPageProps {
  /** Controlled open conversation (the route passes ?open=). */
  openId?: string | null;
  /**
   * Called when the open conversation changes. `replace` is true when moving
   * between open conversations, so Back returns to the list, not the previous
   * message. Omit both props to keep the open state local.
   */
  onOpenChange?: (id: string | null, opts: { replace: boolean }) => void;
}

export const InboxPage: FC<InboxPageProps> = ({ openId, onOpenChange }) => {
  const [tab, setTab] = useState<InboxTab>("needing");
  const [q, setQ] = useState("");
  const [localOpenId, setLocalOpenId] = useState<string | null>(null);
  const [undoState, setUndoState] = useState<UndoState | null>(null);

  const openKey = onOpenChange ? (openId ?? null) : localOpenId;

  const queryClient = useQueryClient();
  const { data, isFetching, isError, error, refetch, fetchNextPage, hasNextPage, isFetchingNextPage } =
    useInboxInfiniteQuery(tab, q);

  const archiveMutation = useArchiveMutation();

  // Realtime subscription
  useInboxRealtimeSync(queryClient);

  // Flatten all pages into one list
  const rows: InboxRow[] = useMemo(
    () => data?.pages.flatMap((p) => p.data) ?? [],
    [data],
  );

  // Selected row (for reading pane)
  const selectedRow = useMemo(
    () => (openKey != null ? rows.find((r) => String(r.id) === openKey) ?? null : null),
    [rows, openKey],
  );
  const selectedId = selectedRow?.id ?? null;

  const setSelectedId = useCallback(
    (id: InboxRow["id"] | null, replace = openKey != null && id != null): void => {
      const next = id == null ? null : String(id);
      if (next === openKey) return;
      if (onOpenChange) onOpenChange(next, { replace });
      else setLocalOpenId(next);
    },
    [openKey, onOpenChange],
  );

  // Close the pane once the list has settled without the open row (archived,
  // or a stale ?open= link). Replace, so Back doesn't reopen a missing row.
  useEffect(() => {
    if (openKey != null && data && !isFetching && !selectedRow) {
      setSelectedId(null, true);
    }
  }, [openKey, data, isFetching, selectedRow, setSelectedId]);

  // Keyboard navigation
  const listRef = useRef<HTMLDivElement>(null);

  const handleArchive = useCallback(
    (id: number): void => {
      const row = rows.find((r) => r.id === id);
      if (!row) return;
      setUndoState({ archivedId: id, archivedName: row.name });
      archiveMutation.mutate({ id, kind: "archive" });
      if (selectedId === id) setSelectedId(null, true);
    },
    [rows, archiveMutation, selectedId, setSelectedId],
  );

  const handleUndo = useCallback((): void => {
    if (!undoState) return;
    archiveMutation.mutate({ id: undoState.archivedId, kind: "unarchive" });
  }, [undoState, archiveMutation]);

  const handleDismissUndo = useCallback((): void => {
    setUndoState(null);
  }, []);

  // Global keyboard handler for j/k navigation
  useEffect(() => {
    const handler = (e: KeyboardEvent): void => {
      // Don't interfere when focus is on an input
      if (
        document.activeElement?.tagName === "INPUT" ||
        document.activeElement?.tagName === "TEXTAREA"
      )
        return;

      const step = (delta: number): void => {
        if (rows.length === 0) return;
        if (selectedId == null) {
          setSelectedId(rows[0]?.id ?? null);
          return;
        }
        const idx = rows.findIndex((r) => r.id === selectedId);
        const next = Math.min(Math.max(idx + delta, 0), rows.length - 1);
        setSelectedId(rows[next]?.id ?? selectedId);
      };

      if (e.key === "j" || e.key === "ArrowDown") {
        e.preventDefault();
        step(1);
      }
      if (e.key === "k" || e.key === "ArrowUp") {
        e.preventDefault();
        step(-1);
      }
      if (e.key === "Escape") {
        setSelectedId(null);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [rows, selectedId, setSelectedId]);

  const isPending = isFetching && !data;

  return (
    <div
      data-testid="inbox.page.root"
      className="flex flex-col h-full"
    >
      {/* Search bar — below lg it steps aside while a conversation is open */}
      <div
        className={cn(
          "items-center gap-2 px-4 py-2 border-b border-line shrink-0 lg:flex",
          selectedRow ? "hidden" : "flex",
        )}
      >
        <Search size={14} strokeWidth={1.8} className="text-t4 shrink-0" aria-hidden="true" />
        <input
          type="search"
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search all mail, people and documents"
          className={cn(
            "flex-1 bg-transparent text-[13px] text-t1 placeholder:text-t4",
            "outline-none border-none",
          )}
          aria-label="Search inbox"
          data-testid="inbox.search.input"
        />
      </div>

      {/* Tabs + Skim button */}
      <div
        className={cn(
          "items-center gap-0 px-4 border-b border-line shrink-0 lg:flex",
          selectedRow ? "hidden" : "flex",
        )}
      >
        {(Object.keys(TAB_LABELS) as InboxTab[]).map((t) => (
          <button
            key={t}
            type="button"
            role="tab"
            aria-selected={tab === t}
            onClick={() => setTab(t)}
            className={cn(
              "px-3 py-2.5 text-[13px] font-[500] border-b-2 transition-colors duration-[--cb-dur]",
              tab === t
                ? "border-accent-cb text-t1"
                : "border-transparent text-t3 hover:text-t2",
            )}
            data-testid={`inbox.tabs.${t}`}
          >
            {TAB_LABELS[t]}
          </button>
        ))}

        {/* Skim the long tail — stub */}
        <button
          type="button"
          className={cn(
            "ml-auto flex items-center gap-1.5 px-2 py-1.5 rounded-cb-1",
            "text-[11.5px] text-t3 font-[500]",
            "hover:text-t2 hover:bg-surface-1 transition-colors duration-[--cb-dur]",
          )}
          aria-label="Skim the long tail (coming soon)"
          data-testid="inbox.skim.button"
          title="Skim the long tail — coming in the next step"
        >
          <Layers size={13} strokeWidth={1.8} aria-hidden="true" />
          Skim long tail
        </button>
      </div>

      {/* Main split: list | reading pane */}
      <div className="flex flex-1 min-h-0 overflow-hidden">
        {/* List pane */}
        <div
          data-testid="inbox.list.pane"
          className={cn(
            "flex-col min-h-0 flex-1",
            // lg+: fixed 400px column beside the reading pane, open or not.
            // Below lg: full width, hidden while a conversation is open.
            "lg:flex lg:flex-none lg:w-[400px] lg:border-r lg:border-line",
            selectedRow ? "hidden" : "flex",
          )}
        >
          {/* Loading */}
          {isPending && (
            <div
              data-testid="inbox.list.loading"
              className="flex-1 overflow-y-auto"
              aria-busy="true"
              aria-label="Loading messages"
            >
              {Array.from({ length: SKELETON_COUNT }).map((_, i) => (
                <SkeletonRow key={i} />
              ))}
            </div>
          )}

          {/* Error */}
          {isError && !isPending && (
            <div
              data-testid="inbox.list.error"
              className="flex-1 flex flex-col items-center justify-center gap-3 p-6 text-center"
            >
              <Visor state="asleep" size={32} label="Error loading inbox" />
              <p className="text-[13px] text-t2">
                {error instanceof Error ? error.message : "Something went wrong"}
              </p>
              <button
                type="button"
                onClick={() => refetch()}
                className={cn(
                  "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
                  "bg-surface-1 border border-line text-[12.5px] text-t2",
                  "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
                )}
                data-testid="inbox.list.retry"
              >
                <RotateCcw size={12} strokeWidth={2} aria-hidden="true" />
                Retry
              </button>
            </div>
          )}

          {/* List */}
          {!isPending && !isError && (
            <>
              <div
                ref={listRef}
                data-testid="inbox.list.root"
                className="flex-1 overflow-y-auto"
                role="listbox"
                aria-label={`${TAB_LABELS[tab]} messages`}
                aria-multiselectable={false}
              >
                {rows.length === 0 ? (
                  <div
                    data-testid="inbox.list.empty"
                    className="flex flex-col items-center justify-center h-full gap-3 p-6 text-center"
                  >
                    <Visor state="happy" size={32} label="Inbox clear" />
                    <p className="text-[13px] text-t2">
                      {tab === "needing"
                        ? "Nothing needs your attention right now."
                        : "No messages."}
                    </p>
                  </div>
                ) : (
                  rows.map((row) => (
                    <InboxRowComponent
                      key={row.id}
                      row={row}
                      selected={selectedId === row.id}
                      onSelect={(id) => setSelectedId(id)}
                      onArchive={handleArchive}
                    />
                  ))
                )}
              </div>

              {/* Load more */}
              {hasNextPage && (
                <div className="shrink-0 px-4 py-2 border-t border-line">
                  <button
                    type="button"
                    onClick={() => fetchNextPage()}
                    disabled={isFetchingNextPage}
                    className={cn(
                      "w-full py-2 text-[12.5px] text-t3 text-center rounded-cb-1",
                      "hover:bg-surface-1 hover:text-t2 transition-colors duration-[--cb-dur]",
                      "disabled:opacity-40 disabled:pointer-events-none",
                    )}
                    data-testid="inbox.list.loadmore"
                  >
                    {isFetchingNextPage ? "Loading…" : "Load more"}
                  </button>
                </div>
              )}
            </>
          )}

          {/* Undo dock */}
          {undoState && (
            <UndoDock
              archivedName={undoState.archivedName}
              onUndo={handleUndo}
              onDismiss={handleDismissUndo}
            />
          )}
        </div>

        {/* Reading side — always present from lg; below lg only while open */}
        <div
          data-testid="inbox.reading-pane.container"
          className={cn("flex-1 min-w-0", selectedRow ? "block" : "hidden lg:block")}
        >
          {selectedRow ? (
            <ReadingPane
              row={selectedRow}
              backLabel={TAB_LABELS[tab]}
              onClose={() => setSelectedId(null)}
              onArchive={handleArchive}
            />
          ) : (
            <EmptyPane />
          )}
        </div>
      </div>
    </div>
  );
};
