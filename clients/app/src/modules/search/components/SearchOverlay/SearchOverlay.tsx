import { useNavigate } from "@tanstack/react-router";
import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";

import { Button, KbdHint, Overlay, SearchField, SkeletonList, Text } from "~/lib/ui";

import { useInstantSearch, useSearchRestart, type SearchHit } from "../../api/queries";
import { HitRow, hitStream } from "../../hit-row";
import {
  chipLabel,
  forgetSearches,
  MAX_QUERY,
  recentSearches,
  rememberSearch,
  removeFilter,
  scrollIntoView,
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

/** Pause after the last keystroke before asking the server. */
const DEBOUNCE_MS = 120;

export interface SearchOverlayProps {
  workspaceId: string;
  open: boolean;
  /** What the field starts with (⌘K hands its query over). */
  initialQuery?: string | undefined;
  onClose: () => void;
}

/**
 * `/`: search from anywhere. A field with the filters it understood as
 * chips, instant rows by relevance with the matched words marked, and the
 * keys: ↑ ↓ move, ↵ opens in the inbox reader, ⌘↵ shows all results.
 */
export function SearchOverlay({
  workspaceId,
  open,
  initialQuery = "",
  onClose,
}: SearchOverlayProps) {
  const { t } = useSearchT();
  return (
    <Overlay open={open} onClose={onClose} label={t("overlay.label")}>
      {/* Mounted only while open, so every opening starts from its own query. */}
      <OverlayContent workspaceId={workspaceId} initialQuery={initialQuery} onClose={onClose} />
    </Overlay>
  );
}

function OverlayContent({
  workspaceId,
  initialQuery,
  onClose,
}: {
  workspaceId: string;
  initialQuery: string;
  onClose: () => void;
}) {
  const { t, language } = useSearchT();
  const navigate = useNavigate();
  const listId = useId();
  const input = useRef<HTMLInputElement>(null);
  const [text, setText] = useState(initialQuery);
  const [active, setActive] = useState(0);
  const [recents, setRecents] = useState(recentSearches);
  const typed = text.trim();
  const debounced = useDebounced(text, DEBOUNCE_MS);
  const settled = debounced.trim() === typed;
  const search = useInstantSearch(workspaceId, debounced);
  const restart = useSearchRestart(workspaceId);

  const data = typed === "" ? undefined : search.data;
  const hits = data?.hits ?? [];
  const activeIndex = Math.min(active, Math.max(hits.length - 1, 0));
  const optionId = (index: number) => `${listId}-option-${String(index)}`;
  const filters = data?.query.filters ?? [];

  const activeId = optionId(activeIndex);
  useEffect(() => {
    scrollIntoView(activeId);
  }, [activeId, hits.length]);

  const showAll = () => {
    rememberSearch(text);
    onClose();
    void navigate({
      to: "/w/$workspaceId/search",
      params: { workspaceId },
      search: typed === "" ? {} : { q: typed },
    });
  };
  const openHit = (hit: SearchHit) => {
    rememberSearch(text);
    onClose();
    void navigate({
      to: `/w/${workspaceId}/inbox/${hitStream(hit)}/${hit.record.id}`,
    });
  };
  const move = (by: number) => {
    if (hits.length > 0) {
      setActive((activeIndex + by + hits.length) % hits.length);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.nativeEvent.isComposing) {
      return;
    }
    const key = event.key.toLowerCase();
    if (key === "arrowdown" || (event.ctrlKey && key === "j")) {
      event.preventDefault();
      move(1);
    } else if (key === "arrowup" || (event.ctrlKey && key === "k")) {
      event.preventDefault();
      move(-1);
    } else if (key === "enter") {
      event.preventDefault();
      const hit = hits[activeIndex];
      if (event.metaKey || event.ctrlKey || hit === undefined) {
        if (typed !== "") {
          showAll();
        }
      } else {
        openHit(hit);
      }
    }
  };

  const failed = typed !== "" && search.isError;
  const loading = typed !== "" && data === undefined && !failed;
  const empty = data !== undefined && hits.length === 0 && settled && !search.isFetching;

  return (
    <>
      <SearchField
        ref={input}
        variant="overlay"
        label={t("field.label")}
        placeholder={t("field.placeholder")}
        value={text}
        maxLength={MAX_QUERY + 1}
        onValueChange={(value) => {
          setText(value);
          setActive(0);
        }}
        onClear={() => {
          setText("");
          setActive(0);
        }}
        keys="escape"
        data-autofocus
        role="combobox"
        aria-expanded={hits.length > 0}
        aria-controls={hits.length > 0 ? listId : undefined}
        aria-autocomplete="list"
        aria-activedescendant={hits.length > 0 ? optionId(activeIndex) : undefined}
        onKeyDown={onKeyDown}
        chips={filters.map((filter) => ({
          id: `${filter.operator}:${filter.value}`,
          label: chipLabel(t, filter),
          onRemove: () => {
            setText(removeFilter(text, filter));
            setActive(0);
            // The chip is gone: keep typing and the arrow keys where they were.
            input.current?.focus();
          },
        }))}
      />
      {typed === "" ? (
        <>
          <RecentSearches
            t={t}
            items={recents}
            onPick={setText}
            onClear={() => {
              forgetSearches();
              setRecents([]);
            }}
          />
          <SearchTips t={t} />
        </>
      ) : null}
      {data?.indexing === true ? <IndexingNote t={t} /> : null}
      {data === undefined ? null : <Warnings t={t} warnings={data.query.warnings} />}
      {data === undefined ? null : <ResultCount t={t} count={hits.length} />}
      {loading ? <SkeletonList rows={3} /> : null}
      {failed ? <SearchProblem t={t} error={search.error} compact onRetry={restart} /> : null}
      {empty ? <NoResults t={t} query={typed} hasFilters={filters.length > 0} /> : null}
      {hits.length > 0 ? (
        <div
          id={listId}
          role="listbox"
          aria-label={t("overlay.results")}
          className="flex flex-col gap-0-5 p-1-5"
        >
          {hits.map((hit, index) => (
            <HitRow
              key={hit.record.id}
              hit={hit}
              t={t}
              language={language}
              id={optionId(index)}
              option
              selected={index === activeIndex}
              onSelect={() => {
                openHit(hit);
              }}
            />
          ))}
        </div>
      ) : null}
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 px-4 py-2">
        <Hint keys="arrowup" label={t("overlay.hintMove")} extra="arrowdown" />
        <Hint keys="enter" label={t("overlay.hintOpen")} />
        <Button variant="ghost" keys="mod+enter" onClick={showAll} disabled={typed === ""}>
          {t("overlay.hintAll")}
        </Button>
        <Hint keys="escape" label={t("overlay.hintClose")} />
      </div>
    </>
  );
}

function Hint({ keys, extra, label }: { keys: string; extra?: string; label: string }) {
  return (
    <Text as="span" size="xs" tone="tertiary" className="inline-flex items-center gap-1-5">
      <KbdHint keys={keys} />
      {extra === undefined ? null : <KbdHint keys={extra} />}
      {label}
    </Text>
  );
}
