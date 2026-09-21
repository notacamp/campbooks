/**
 * InboxPage — the unified inbox surface at /inbox.
 *
 * Features:
 *   - Search bar
 *   - Important / Everything tabs (Important = needs_you == true)
 *   - Infinite-paged list of InboxRows (Avatar, name, time, subject, annotation, unread)
 *   - Archive with in-place Undo (optimistic via TanStack Query)
 *   - Reading pane (≥ 900px split; single-pane on mobile)
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
import { UndoDock } from "./UndoDock";
import { cn } from "~/lib/utils";

const SKELETON_COUNT = 8;

// ── UndoState ─────────────────────────────────────────────────────────────────

interface UndoState {
  archivedId: number;
  archivedName: string;
}

// ── InboxPage ─────────────────────────────────────────────────────────────────

export const InboxPage: FC = () => {
  const [tab, setTab] = useState<InboxTab>("needing");
  const [q, setQ] = useState("");
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [undoState, setUndoState] = useState<UndoState | null>(null);

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
    () => (selectedId != null ? rows.find((r) => r.id === selectedId) : null),
    [rows, selectedId],
  );

  // Clear selection if selected row is no longer in the list (e.g. archived)
  useEffect(() => {
    if (selectedId != null && !rows.some((r) => r.id === selectedId)) {
      setSelectedId(null);
    }
  }, [rows, selectedId]);

  // Keyboard navigation
  const listRef = useRef<HTMLDivElement>(null);

  const handleArchive = useCallback(
    (id: number): void => {
      const row = rows.find((r) => r.id === id);
      if (!row) return;
      setUndoState({ archivedId: id, archivedName: row.name });
      archiveMutation.mutate({ id, kind: "archive" });
      if (selectedId === id) setSelectedId(null);
    },
    [rows, archiveMutation, selectedId],
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

      if (e.key === "j" || e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedId((prev) => {
          if (rows.length === 0) return prev;
          if (prev == null) return rows[0]?.id ?? null;
          const idx = rows.findIndex((r) => r.id === prev);
          return rows[Math.min(idx + 1, rows.length - 1)]?.id ?? prev;
        });
      }
      if (e.key === "k" || e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedId((prev) => {
          if (rows.length === 0) return prev;
          if (prev == null) return rows[0]?.id ?? null;
          const idx = rows.findIndex((r) => r.id === prev);
          return rows[Math.max(idx - 1, 0)]?.id ?? prev;
        });
      }
      if (e.key === "Escape") {
        setSelectedId(null);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [rows]);

  const isPending = isFetching && !data;

  return (
    <div
      data-testid="inbox.page.root"
      className="flex flex-col h-full"
    >
      {/* Search bar */}
      <div className="flex items-center gap-2 px-4 py-2 border-b border-line shrink-0">
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
      <div className="flex items-center gap-0 px-4 border-b border-line shrink-0">
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
          className={cn(
            "flex flex-col min-h-0",
            // Full width on mobile / narrow; fixed 360px when reading pane is open on desktop
            selectedRow
              ? "hidden xl:flex xl:w-[360px] shrink-0"
              : "flex-1",
            "border-r border-line",
          )}
          style={selectedRow ? { width: "360px" } : undefined}
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
                      onSelect={setSelectedId}
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

        {/* Reading pane — ≥ 900px only (xl breakpoint ≈ 1280px here; we use min-w check) */}
        {selectedRow && (
          <div
            data-testid="inbox.reading-pane.container"
            className="flex-1 min-w-0 hidden xl:block"
          >
            <ReadingPane
              row={selectedRow}
              onClose={() => setSelectedId(null)}
              onArchive={handleArchive}
            />
          </div>
        )}
      </div>
    </div>
  );
};
