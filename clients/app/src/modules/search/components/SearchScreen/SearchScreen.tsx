import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  Chip,
  EmptyState,
  Heading,
  KbdHint,
  Line,
  ListPane,
  PaneHeader,
  ReaderPane,
  SearchField,
  Segmented,
  SkeletonList,
  Spacer,
} from "~/lib/ui";
import {
  ConversationReader,
  renderReplyComposer,
  useConversationSummaries,
  useInboxActions,
  type ConversationSummary,
} from "~/modules/conversations";

import {
  useSearchResults,
  useSearchRestart,
  type SearchHit,
  type SearchSort,
} from "../../api/queries";
import { HitRow, hitStream } from "../../hit-row";
import {
  chipLabel,
  forgetSearches,
  hasFilter,
  MAX_QUERY,
  recentSearches,
  rememberSearch,
  removeFilter,
  scrollIntoView,
  toggleFilter,
} from "../../model";
import {
  IndexingNote,
  NoResults,
  RecentSearches,
  ResultCount,
  SearchProblem,
  SearchTips,
  Warnings,
} from "../../states";
import { useDebounced } from "../../use-debounced";
import { useSearchT } from "../../use-search-t";

const DEBOUNCE_MS = 250;

/** The keys the empty reader teaches. */
const READER_HINTS = ["list.next", "list.previous", "list.open", "item.done"] as const;

/** Where list and reader become separate screens (the shell's large breakpoint). */
const NARROW = "(width < 64rem)";

function isNarrow(): boolean {
  return typeof globalThis.matchMedia === "function" && globalThis.matchMedia(NARROW).matches;
}

const rowId = (conversationId: string) => `search-hit-${conversationId}`;

export interface SearchScreenProps {
  workspaceId: string;
  /** The query in the address. */
  query: string;
  sort: SearchSort;
  /** The conversation open in the reader, if any. */
  conversationId?: string | undefined;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/**
 * `/w/:workspace/search?q=`: the field with its chips, the hits newest first
 * (or most relevant) with the matched words marked, and the reader beside
 * them on wide screens. J K ↵ E S U Z act as in the inbox; on a phone the
 * reader is its own screen.
 */
export function SearchScreen({ workspaceId, query, sort, conversationId, now }: SearchScreenProps) {
  const { t, language } = useSearchT();
  const { t: tShortcuts } = useTranslation("shortcuts");
  const { t: tConversations } = useTranslation("conversations");
  const navigate = useNavigate();
  const field = useRef<HTMLInputElement>(null);
  const [draft, setDraft] = useState(query);
  const [cursor, setCursor] = useState<string | undefined>();
  const [recents, setRecents] = useState(recentSearches);
  const pushed = useRef(query);
  // An empty search page opens ready to type; the reader's page does not.
  const startsEmpty = useRef(query === "" && conversationId === undefined);
  useEffect(() => {
    if (startsEmpty.current) {
      field.current?.focus();
    }
  }, []);
  const typed = draft.trim();
  const debounced = useDebounced(draft, DEBOUNCE_MS).trim();

  const results = useSearchResults(workspaceId, debounced, sort);
  const restart = useSearchRestart(workspaceId);
  const actions = useInboxActions(workspaceId);

  const pages = results.data?.pages ?? [];
  const hits = pages.flatMap((page) => page.hits);
  const ids = hits.map((hit) => hit.record.id);
  const summaries = useConversationSummaries(workspaceId, ids);
  const first = pages[0];
  const filters = first?.query.filters ?? [];
  const warnings = first?.query.warnings ?? [];
  const indexing = pages.some((page) => page.indexing);

  // The address follows the field (once typing settles); a query put there
  // from outside (a recent search, the overlay) comes back into the field.
  useEffect(() => {
    if (query !== pushed.current) {
      pushed.current = query;
      setDraft(query);
    }
  }, [query]);
  useEffect(() => {
    if (debounced !== pushed.current.trim()) {
      pushed.current = debounced;
      void navigate({
        to:
          conversationId === undefined
            ? "/w/$workspaceId/search"
            : "/w/$workspaceId/search/$conversationId",
        params: { workspaceId, ...(conversationId === undefined ? {} : { conversationId }) },
        search: {
          ...(debounced === "" ? {} : { q: debounced }),
          ...(sort === "relevance" ? { sort } : {}),
        },
        replace: true,
      });
    }
  }, [debounced, navigate, workspaceId, conversationId, sort]);

  const go = (id?: string, nextSort: SearchSort = sort) => {
    const search = {
      ...(typed === "" ? {} : { q: typed }),
      ...(nextSort === "relevance" ? { sort: nextSort } : {}),
    };
    void navigate(
      id === undefined
        ? { to: "/w/$workspaceId/search", params: { workspaceId }, search }
        : {
            to: "/w/$workspaceId/search/$conversationId",
            params: { workspaceId, conversationId: id },
            search,
          },
    );
  };
  const open = (id: string) => {
    rememberSearch(draft);
    setCursor(id);
    go(id);
  };
  const toList = () => {
    go();
  };

  const current = conversationId ?? cursor ?? ids[0];
  const step = (by: number) => {
    // The first J or K in a list lands on its first row.
    const index =
      current === undefined || (conversationId === undefined && cursor === undefined)
        ? -by
        : ids.indexOf(current);
    const next = ids[Math.min(ids.length - 1, Math.max(0, index + by))];
    if (next === undefined) {
      return;
    }
    if (conversationId === undefined) {
      setCursor(next);
      scrollIntoView(rowId(next));
    } else {
      open(next);
    }
  };

  /** Done: archive, then the next hit (or the list, on phones). */
  const done = (conversation: ConversationSummary) => {
    const index = ids.indexOf(conversation.id);
    const following = ids[index + 1] ?? ids[index - 1];
    actions.done(conversation);
    if (conversationId === conversation.id) {
      if (following === undefined || isNarrow()) {
        toList();
      } else {
        open(following);
      }
    } else {
      setCursor(following);
    }
  };

  const listActive = conversationId === undefined;
  const summaryOf = (id: string | undefined) => (id === undefined ? undefined : summaries.get(id));
  useShortcut("search.open", () => {
    field.current?.focus();
    field.current?.select();
  });
  useShortcut(
    "list.next",
    () => {
      step(1);
    },
    { enabled: listActive },
  );
  useShortcut(
    "list.previous",
    () => {
      step(-1);
    },
    { enabled: listActive },
  );
  useShortcut(
    "list.open",
    () => {
      if (current !== undefined) {
        open(current);
      }
    },
    { enabled: listActive },
  );
  useShortcut(
    "item.done",
    () => {
      const conversation = summaryOf(current);
      if (conversation !== undefined) {
        done(conversation);
      }
    },
    { enabled: listActive },
  );
  useShortcut(
    "conversations.star",
    () => {
      const conversation = summaryOf(current);
      if (conversation !== undefined) {
        actions.toggleStar(conversation);
      }
    },
    { enabled: listActive },
  );
  useShortcut(
    "conversations.mark_unread",
    () => {
      const conversation = summaryOf(current);
      if (conversation !== undefined) {
        actions.toggleRead(conversation);
      }
    },
    { enabled: listActive },
  );

  const onFieldKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter" && !event.nativeEvent.isComposing) {
      event.preventDefault();
      // Leave the field for the list: J and K move from here.
      field.current?.blur();
      if (ids[0] !== undefined) {
        setCursor(ids[0]);
      }
    } else if (event.key === "Escape") {
      field.current?.blur();
    }
  };

  const toggle = (operator: string, value: string) => {
    setDraft(toggleFilter(draft, filters, operator, value));
  };

  const failed = results.isError;
  const loading = debounced !== "" && results.data === undefined && !failed;
  const empty =
    results.data !== undefined && hits.length === 0 && debounced === typed && !results.isFetching;
  const readerHidden = conversationId === undefined;

  return (
    <>
      <ListPane label={t("page.list")} narrow={readerHidden ? "shown" : "hidden"}>
        <PaneHeader>
          <Heading level={1} size="pane">
            {t("page.title")}
          </Heading>
          <Spacer />
          <Segmented
            label={t("page.order")}
            value={sort}
            onChange={(value) => {
              go(conversationId, value);
            }}
            options={[
              { value: "newest", label: t("page.newest") },
              { value: "relevance", label: t("page.relevance") },
            ]}
          />
        </PaneHeader>
        <div className="flex flex-col gap-2 px-3 pt-2">
          <SearchField
            ref={field}
            label={t("field.label")}
            placeholder={t("field.placeholder")}
            value={draft}
            maxLength={MAX_QUERY + 1}
            onValueChange={setDraft}
            onClear={() => {
              setDraft("");
              field.current?.focus();
            }}
            keys="/"
            onKeyDown={onFieldKeyDown}
            chips={filters.map((filter) => ({
              id: `${filter.operator}:${filter.value}`,
              label: chipLabel(t, filter),
              onRemove: () => {
                setDraft(removeFilter(draft, filter));
                field.current?.focus();
              },
            }))}
          >
            <div className="flex flex-wrap gap-2" role="group" aria-label={t("page.quick")}>
              <Chip
                pressed={hasFilter(filters, "has", "attachment")}
                onClick={() => {
                  toggle("has", "attachment");
                }}
              >
                {t("page.attachmentToggle")}
              </Chip>
              <Chip
                pressed={hasFilter(filters, "is", "unread")}
                onClick={() => {
                  toggle("is", "unread");
                }}
              >
                {t("page.unreadToggle")}
              </Chip>
            </div>
          </SearchField>
        </div>
        {actions.failedOn === undefined ? null : (
          <div className="p-2">
            <Banner
              tone="danger"
              role="alert"
              title={tConversations("error.action", { subject: actions.failedOn })}
              action={
                <Button onClick={actions.dismissFailure}>{tShortcuts("overlay.close")}</Button>
              }
            />
          </div>
        )}
        {debounced === "" && typed === "" ? (
          <>
            <RecentSearches
              t={t}
              items={recents}
              onPick={setDraft}
              onClear={() => {
                forgetSearches();
                setRecents([]);
              }}
            />
            <SearchTips t={t} />
          </>
        ) : null}
        {indexing ? <IndexingNote t={t} /> : null}
        <Warnings t={t} warnings={warnings} />
        {results.data === undefined ? null : <ResultCount t={t} count={hits.length} />}
        {loading ? <SkeletonList rows={6} /> : null}
        {failed ? (
          <SearchProblem
            t={t}
            error={results.error}
            // An out-of-date cursor is fixed by starting over from the first page.
            onRetry={restart}
          />
        ) : null}
        {empty ? <NoResults t={t} query={debounced} hasFilters={filters.length > 0} /> : null}
        {hits.length > 0 ? (
          <div className="flex flex-col gap-0-5 p-1-5">
            {hits.map((hit: SearchHit) => {
              const id = hit.record.id;
              const summary = summaries.get(id);
              return (
                <HitRow
                  key={id}
                  hit={hit}
                  t={t}
                  language={language}
                  id={rowId(id)}
                  unread={summary === undefined ? undefined : summary.unread_count > 0}
                  starred={summary?.starred}
                  current={id === conversationId}
                  selected={id === cursor && id !== conversationId}
                  now={now}
                  onSelect={() => {
                    open(id);
                  }}
                />
              );
            })}
            {results.hasNextPage ? (
              <div className="flex justify-center p-2">
                <Button
                  variant="ghost"
                  disabled={results.isFetchingNextPage}
                  onClick={() => {
                    void results.fetchNextPage();
                  }}
                >
                  {results.isFetchingNextPage ? t("page.loadingMore") : t("page.more")}
                </Button>
              </div>
            ) : null}
          </div>
        ) : null}
      </ListPane>
      <ReaderPane label={t("page.reader")} narrow={readerHidden ? "hidden" : "shown"}>
        {conversationId === undefined ? (
          <div className="flex flex-col items-center gap-2 p-6">
            <EmptyState title={t("page.readerTitle")} visor="idle" visorSize="lg">
              {t("page.readerBody")}
            </EmptyState>
            <div className="flex w-full max-w-measure flex-col">
              {READER_HINTS.map((id) => (
                <Line key={id} trailing={<KbdHint shortcut={id} announce />}>
                  {tShortcuts(id)}
                </Line>
              ))}
            </div>
          </div>
        ) : (
          <ConversationReader
            // Each conversation opens folded, with its images blocked again.
            key={conversationId}
            workspaceId={workspaceId}
            conversationId={conversationId}
            stream={hitStreamOf(hits, conversationId)}
            backLabel={t("page.back")}
            actions={actions}
            onBack={toList}
            onPrevious={() => {
              step(-1);
            }}
            onNext={() => {
              step(1);
            }}
            onDone={done}
            renderReplySlot={renderReplyComposer}
            now={now}
          />
        )}
      </ReaderPane>
    </>
  );
}

function hitStreamOf(hits: readonly SearchHit[], id: string) {
  const hit = hits.find((candidate) => candidate.record.id === id);
  return hit === undefined ? "primary" : hitStream(hit);
}
