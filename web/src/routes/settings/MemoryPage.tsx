/**
 * MemoryPage — Settings → Scout's memory at /settings/memory.
 *
 * Scout's memory re-expresses the inbox/AI settings as editable sentences
 * derived on demand from behaviour records (rules, tags, learned habits, etc.).
 *
 * Endpoints:
 *   GET    /api/app/settings/memory                         — catalog
 *   POST   /api/app/settings/memory/teach                   — teach a sentence
 *   POST   /api/app/settings/memory/entries/:id/confirm     — confirm a habit
 *   DELETE /api/app/settings/memory/entries/:id             — remove an entry
 *
 * The catalog is grouped by facet (filing/stack/replies/streams/people/style).
 * Each entry carries its origin (taught/learned/default) and available actions.
 */
import { type FC, useState, useCallback } from "react";
import { RotateCcw, Check, Plus, Trash2, X } from "lucide-react";
import { Visor } from "~/lib/ui";
import { ApiError } from "~/lib/api";
import { cn } from "~/lib/utils";
import {
  useMemoryQuery,
  type MemoryEntryData,
  type MemoryFacet,
  type MemorySentence,
} from "~/modules/settings/api/use-memory-query";
import {
  useMemoryTeachMutation,
  useMemoryConfirmMutation,
  useMemoryRemoveMutation,
} from "~/modules/settings/api/use-memory-mutation";

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Normalise the server sentence shape to a plain string. */
const plainSentence = (sentence: string | MemorySentence): string => {
  if (typeof sentence === "string") return sentence;
  return sentence.plain;
};

// ── Facet config ──────────────────────────────────────────────────────────────

const FACET_LABELS: Record<MemoryFacet, string> = {
  filing:      "Filing",
  stack:       "Inbox stack",
  replies:     "Replies",
  streams:     "Streams",
  people:      "People",
  style:       "Writing style",
  automations: "Automations",
};

const FACETS: MemoryFacet[] = [
  "filing",
  "stack",
  "replies",
  "streams",
  "people",
  "style",
  "automations",
];

const ORIGIN_LABELS: Record<string, string> = {
  taught:  "Taught",
  learned: "Learned",
  default: "Default",
};

const ORIGIN_CLASSES: Record<string, string> = {
  taught:  "text-accent-cb bg-accent-cb/10",
  learned: "text-t2 bg-surface-2 border border-line",
  default: "text-t4 bg-surface-2 border border-line",
};

// ── Entry row ─────────────────────────────────────────────────────────────────

interface EntryRowProps {
  entry: MemoryEntryData;
}

const EntryRow: FC<EntryRowProps> = ({ entry }) => {
  const confirmMutation = useMemoryConfirmMutation();
  const removeMutation  = useMemoryRemoveMutation();
  const [confirmDelete, setConfirmDelete] = useState(false);

  const canConfirm = entry.actions.includes("confirm");
  const canRemove  = entry.actions.includes("remove");

  const isActing = confirmMutation.isPending || removeMutation.isPending;

  return (
    <li className="flex items-start gap-3 py-3 border-b border-line last:border-b-0">
      {/* Origin badge */}
      <span
        className={cn(
          "mt-0.5 shrink-0 inline-flex px-1.5 py-0.5 rounded-cb-1 text-[10.5px] font-[500]",
          ORIGIN_CLASSES[entry.origin] ?? ORIGIN_CLASSES.default,
        )}
        title={entry.origin_detail ?? undefined}
      >
        {ORIGIN_LABELS[entry.origin] ?? entry.origin}
      </span>

      {/* Sentence */}
      <div className="flex-1 min-w-0">
        {/* Rendered as escaped plain text: the server sends a plain sentence
            (no HTML), and it can embed user-derived values (tag names, taught
            text), so it must never go through dangerouslySetInnerHTML. */}
        <span className="text-[13px] text-t1 leading-relaxed">
          {plainSentence(entry.sentence)}
        </span>
        {entry.origin_detail && (
          <p className="text-[11.5px] text-t4 mt-0.5">{entry.origin_detail}</p>
        )}
        {confirmMutation.error && (
          <p className="text-[11.5px] text-danger mt-1" role="alert">
            {confirmMutation.error instanceof ApiError
              ? confirmMutation.error.message
              : "Something went wrong"}
          </p>
        )}
        {removeMutation.error && (
          <p className="text-[11.5px] text-danger mt-1" role="alert">
            {removeMutation.error instanceof ApiError
              ? removeMutation.error.message
              : "Could not remove entry"}
          </p>
        )}
      </div>

      {/* Actions */}
      {(canConfirm || canRemove) && (
        <div className="flex items-center gap-1 shrink-0 mt-0.5">
          {canConfirm && !confirmDelete && (
            <button
              type="button"
              onClick={() => confirmMutation.mutate({ id: entry.id })}
              disabled={isActing}
              className={cn(
                "flex items-center gap-1 px-2 py-1 rounded-cb-1 text-[11.5px] font-[500]",
                "text-accent-cb bg-accent-cb/10",
                "hover:bg-accent-cb/20 transition-colors duration-[--cb-dur]",
                "disabled:opacity-60 disabled:cursor-not-allowed",
              )}
              aria-label={`Confirm: ${plainSentence(entry.sentence)}`}
            >
              {confirmMutation.isPending ? (
                "Confirming…"
              ) : (
                <>
                  <Check size={11} strokeWidth={2.5} aria-hidden="true" />
                  Keep
                </>
              )}
            </button>
          )}

          {canRemove && (
            <>
              {confirmDelete ? (
                <>
                  <button
                    type="button"
                    onClick={() => removeMutation.mutate({ id: entry.id })}
                    disabled={isActing}
                    className="text-[11.5px] text-danger px-2 py-1 rounded-cb-1 hover:bg-danger/10 transition-colors duration-[--cb-dur]"
                  >
                    {removeMutation.isPending ? "Removing…" : "Confirm remove"}
                  </button>
                  <button
                    type="button"
                    onClick={() => setConfirmDelete(false)}
                    className="p-1.5 rounded-cb-1 text-t4 hover:text-t2 hover:bg-surface-2 transition-colors duration-[--cb-dur]"
                    aria-label="Cancel remove"
                  >
                    <X size={12} strokeWidth={2} aria-hidden="true" />
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  onClick={() => setConfirmDelete(true)}
                  disabled={isActing}
                  className="p-1.5 rounded-cb-1 text-t4 hover:text-danger hover:bg-danger/10 transition-colors duration-[--cb-dur] disabled:opacity-60"
                  aria-label={`Remove: ${plainSentence(entry.sentence)}`}
                >
                  <Trash2 size={12} strokeWidth={2} aria-hidden="true" />
                </button>
              )}
            </>
          )}
        </div>
      )}
    </li>
  );
};

// ── Facet section ─────────────────────────────────────────────────────────────

interface FacetSectionProps {
  facet: MemoryFacet;
  entries: MemoryEntryData[];
  count: number;
}

const FacetSection: FC<FacetSectionProps> = ({ facet, entries, count }) => {
  if (count === 0) return null;

  return (
    <section className="bg-surface-1 cb-raised rounded-cb-2 divide-y divide-line mb-4">
      <div className="px-4 py-3 sm:px-5 flex items-center justify-between">
        <h2 className="text-[13.5px] font-[600] text-t1">{FACET_LABELS[facet]}</h2>
        <span className="text-[12px] text-t4">{count}</span>
      </div>

      <ul className="px-4 sm:px-5" aria-label={`${FACET_LABELS[facet]} entries`}>
        {entries.map((entry) => (
          <EntryRow key={entry.id} entry={entry} />
        ))}
      </ul>
    </section>
  );
};

// ── Teach panel ───────────────────────────────────────────────────────────────

interface TeachPanelProps {
  onDone: () => void;
}

const TeachPanel: FC<TeachPanelProps> = ({ onDone }) => {
  const teachMutation = useMemoryTeachMutation();
  const [sentence, setSentence] = useState("");
  const [result,   setResult]   = useState<{ created: boolean; reason?: string } | null>(null);

  const handleTeach = useCallback((): void => {
    if (!sentence.trim()) return;
    teachMutation.mutate(
      { sentence: sentence.trim() },
      {
        onSuccess: (data) => {
          if (data.created) {
            setSentence("");
            setResult(null);
            onDone();
          } else {
            setResult({ created: false, reason: data.reason });
          }
        },
      },
    );
  }, [teachMutation, sentence, onDone]);

  return (
    <div className="flex flex-col gap-3 p-4 bg-surface-2 rounded-cb-1 border border-line mb-4">
      <p className="text-[13px] font-[600] text-t1">Teach Scout something</p>
      <p className="text-[12px] text-t3 leading-relaxed">
        Write a plain-language rule or preference and Scout will turn it into the
        right setting. Example: &ldquo;Always tag emails from @acme.com as Priority&rdquo;.
      </p>

      <textarea
        value={sentence}
        onChange={(e) => setSentence(e.target.value)}
        placeholder="Type a rule or preference…"
        rows={3}
        className={cn(
          "w-full px-3 py-2 rounded-cb-1 text-[13px] resize-y",
          "bg-surface-3 border border-line",
          "text-t1 placeholder:text-t4",
          "focus:outline-none focus:ring-2 focus:ring-[--ring]",
          "transition-colors duration-[--cb-dur]",
        )}
        aria-label="Sentence to teach Scout"
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            handleTeach();
          }
        }}
      />

      {result && !result.created && (
        <p className="text-[12px] text-t3 leading-relaxed bg-surface-3 px-3 py-2 rounded-cb-1 border border-line">
          {result.reason ?? "Scout didn't understand that — try rephrasing."}
        </p>
      )}

      {teachMutation.error && (
        <p className="text-[12px] text-danger" role="alert">
          {teachMutation.error instanceof ApiError
            ? teachMutation.error.message
            : "Something went wrong"}
        </p>
      )}

      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={handleTeach}
          disabled={teachMutation.isPending || !sentence.trim()}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1 text-[12.5px] font-[500]",
            "bg-[--primary] text-[--primary-foreground]",
            "disabled:opacity-60 disabled:cursor-not-allowed",
            "transition-colors duration-[--cb-dur]",
          )}
        >
          {teachMutation.isPending ? "Teaching…" : "Teach Scout"}
        </button>
        <button
          type="button"
          onClick={() => { setSentence(""); setResult(null); onDone(); }}
          disabled={teachMutation.isPending}
          className={cn(
            "px-3 py-1.5 rounded-cb-1 text-[12.5px] text-t3",
            "bg-surface-1 border border-line",
            "hover:bg-surface-2 transition-colors duration-[--cb-dur]",
            "disabled:opacity-60",
          )}
        >
          Cancel
        </button>
      </div>
    </div>
  );
};

// ── MemoryPage ────────────────────────────────────────────────────────────────

export const MemoryPage: FC = () => {
  const [activeFacet, setActiveFacet] = useState<MemoryFacet | null>(null);
  const [showTeach,   setShowTeach]   = useState(false);
  const [search,      setSearch]      = useState("");

  const { data, isFetching, isError, error, refetch } = useMemoryQuery(activeFacet);

  // ── Loading ────────────────────────────────────────────────────────────────

  const isLoading = isFetching && !data;

  if (isLoading) {
    return (
      <div
        data-testid="settings.page.loading"
        className="flex flex-col gap-4 max-w-2xl animate-pulse"
        aria-busy="true"
        aria-label="Loading Scout's memory"
      >
        <div className="h-6 w-48 bg-surface-1 rounded-cb-1" />
        {[0, 1, 2, 3].map((i) => (
          <div key={i} className="h-20 bg-surface-1 rounded-cb-1" />
        ))}
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        data-testid="settings.page.error"
        className="flex flex-col items-center justify-center gap-3 py-16 text-center"
      >
        <Visor state="asleep" size={28} label="Error loading memory" />
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
          data-testid="settings.page.retry"
        >
          <RotateCcw size={12} strokeWidth={2} aria-hidden="true" />
          Retry
        </button>
      </div>
    );
  }

  // ── Filter entries by search ───────────────────────────────────────────────

  const needle = search.trim().toLowerCase();
  const visibleEntries = needle
    ? data.entries.filter((e) =>
        plainSentence(e.sentence).toLowerCase().includes(needle) ||
        (e.origin_detail ?? "").toLowerCase().includes(needle),
      )
    : data.entries;

  // Group entries by facet preserving FACETS order
  const byFacet = FACETS.reduce<Record<string, MemoryEntryData[]>>((acc, f) => {
    acc[f] = visibleEntries.filter((e) => e.facet === f);
    return acc;
  }, {});

  // ── Content ────────────────────────────────────────────────────────────────

  return (
    <div data-testid="settings.page.root" className="max-w-2xl">
      {/* Header */}
      <div className="flex items-start justify-between gap-3 mb-6">
        <div>
          <h1 className="text-[17px] font-[650] text-t1">Scout&apos;s memory</h1>
          <p className="text-[12px] text-t3 mt-0.5">
            Everything Scout has learned about how you work.
            {data.total > 0 && (
              <span className="ml-1">{data.total} entries.</span>
            )}
          </p>
        </div>
        {!showTeach && (
          <button
            type="button"
            onClick={() => setShowTeach(true)}
            className={cn(
              "shrink-0 flex items-center gap-1.5 px-3 py-1.5 rounded-cb-1",
              "text-[12.5px] font-[500] bg-[--primary] text-[--primary-foreground]",
              "transition-colors duration-[--cb-dur]",
            )}
          >
            <Plus size={13} strokeWidth={2.5} aria-hidden="true" />
            Teach Scout
          </button>
        )}
      </div>

      {/* Teach panel */}
      {showTeach && (
        <TeachPanel onDone={() => setShowTeach(false)} />
      )}

      {/* Facet filter chips */}
      <div className="flex flex-wrap gap-1.5 mb-4" role="group" aria-label="Filter by facet">
        <button
          type="button"
          onClick={() => setActiveFacet(null)}
          className={cn(
            "px-2.5 py-1 rounded-cb-1 text-[12px] font-[500]",
            "border transition-colors duration-[--cb-dur]",
            activeFacet === null
              ? "bg-t1 text-surface-3 border-t1"
              : "bg-surface-1 text-t3 border-line hover:bg-surface-2",
          )}
        >
          All
          {data.total > 0 && (
            <span className="ml-1 text-[11px] opacity-70">{data.total}</span>
          )}
        </button>
        {FACETS.filter((f) => (data.facet_counts[f] ?? 0) > 0).map((f) => (
          <button
            key={f}
            type="button"
            onClick={() => setActiveFacet(f === activeFacet ? null : f)}
            className={cn(
              "px-2.5 py-1 rounded-cb-1 text-[12px] font-[500]",
              "border transition-colors duration-[--cb-dur]",
              activeFacet === f
                ? "bg-t1 text-surface-3 border-t1"
                : "bg-surface-1 text-t3 border-line hover:bg-surface-2",
            )}
          >
            {FACET_LABELS[f]}
            <span className="ml-1 text-[11px] opacity-70">{data.facet_counts[f]}</span>
          </button>
        ))}
      </div>

      {/* Search */}
      <div className="relative mb-4">
        <input
          type="search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Filter entries…"
          aria-label="Search memory entries"
          className={cn(
            "w-full px-3 py-2 rounded-cb-1 text-[13px]",
            "bg-surface-1 border border-line",
            "text-t1 placeholder:text-t4",
            "focus:outline-none focus:ring-2 focus:ring-[--ring]",
            "transition-colors duration-[--cb-dur]",
          )}
        />
      </div>

      {/* Empty state */}
      {visibleEntries.length === 0 && (
        <div className="flex flex-col items-center justify-center gap-3 py-16 text-center">
          <Visor state="thinking" size={28} label="No memory entries" />
          <p className="text-[13px] text-t3">
            {search
              ? "No entries match your search."
              : "Scout's memory is empty. Teach it something or process some emails."}
          </p>
        </div>
      )}

      {/* Facet sections */}
      {visibleEntries.length > 0 &&
        FACETS.map((facet) => (
          <FacetSection
            key={facet}
            facet={facet}
            entries={byFacet[facet] ?? []}
            count={byFacet[facet]?.length ?? 0}
          />
        ))}
    </div>
  );
};
