import { useNavigate, useParams } from "@tanstack/react-router";

import { SkeletonList, type MenuOption, type MenuQuery, type MenuSection } from "~/lib/ui";

import { useInstantSearch, useSearchRestart, type SearchHit } from "./api/queries";
import { HitRow, hitStream } from "./hit-row";
import { chipLabel, removeFilter } from "./model";
import { IndexingNote, NoResults, SearchProblem, Warnings } from "./states";
import { useDebounced } from "./use-debounced";
import { useRecentSearches } from "./use-recent";
import { useSearchT } from "./use-search-t";

/** Pause after the last keystroke before asking the server. */
const DEBOUNCE_MS = 120;

/**
 * The command menu's conversations: instant hits by relevance with the
 * matched words marked, the filters understood as chips, and "All results"
 * (⌘↵) at the end. With nothing typed, the device's recent searches, which
 * fill the field. Outside a workspace there is no section. Pass it to
 * `CommandMenu` as `useSection`.
 */
export function useConversationSection({ query, setQuery }: MenuQuery): MenuSection | undefined {
  const { t, language } = useSearchT();
  const navigate = useNavigate();
  const { workspaceId = "" } = useParams({ strict: false });
  const recent = useRecentSearches(workspaceId);
  const typed = query.trim();
  const debounced = useDebounced(query, DEBOUNCE_MS);
  const settled = debounced.trim() === typed;
  const search = useInstantSearch(workspaceId, workspaceId === "" ? "" : debounced);
  const restart = useSearchRestart(workspaceId);

  if (workspaceId === "") {
    return undefined;
  }

  const showAll = () => {
    recent.remember(query);
    void navigate({
      to: "/w/$workspaceId/search",
      params: { workspaceId },
      search: { q: typed },
    });
  };

  if (typed === "") {
    if (recent.items.length === 0) {
      return undefined;
    }
    return {
      group: t("menu.recent"),
      options: [
        ...recent.items.map((item): MenuOption => ({
          id: `recent:${item}`,
          label: item,
          keepOpen: true,
          onSelect: () => {
            setQuery(item);
          },
        })),
        {
          id: "recent:clear",
          label: t("menu.clearRecent"),
          keepOpen: true,
          onSelect: recent.clear,
        },
      ],
    };
  }

  const data = search.data;
  const hits = data?.hits ?? [];
  const filters = data?.query.filters ?? [];
  const failed = search.isError;
  const loading = data === undefined && !failed;
  const empty = data !== undefined && hits.length === 0 && settled && !search.isFetching;

  const openHit = (hit: SearchHit) => {
    recent.remember(query);
    void navigate({ to: `/w/${workspaceId}/inbox/${hitStream(hit)}/${hit.record.id}` });
  };
  const options: MenuOption[] = hits.map((hit) => ({
    id: hit.record.id,
    label: hit.title.map((segment) => segment.text).join(""),
    onSelect: () => {
      openHit(hit);
    },
    render: ({ id, selected, select }) => (
      <HitRow
        hit={hit}
        t={t}
        language={language}
        id={id}
        option
        selected={selected}
        onSelect={select}
      />
    ),
  }));
  if (hits.length > 0) {
    options.push({ id: "all", label: t("menu.all", { query: typed }), onSelect: showAll });
  }

  return {
    group: t("menu.group"),
    options,
    onSubmitAll: showAll,
    pending: loading || !settled,
    chips: filters.map((filter) => ({
      id: `${filter.operator}:${filter.value}`,
      label: chipLabel(t, filter),
      onRemove: () => {
        setQuery(removeFilter(query, filter));
      },
    })),
    status: (
      <>
        {data?.indexing === true ? <IndexingNote t={t} /> : null}
        {data === undefined ? null : <Warnings t={t} warnings={data.query.warnings} />}
        {loading ? <SkeletonList rows={3} /> : null}
        {failed ? <SearchProblem t={t} error={search.error} compact onRetry={restart} /> : null}
        {empty ? <NoResults t={t} query={typed} hasFilters={filters.length > 0} /> : null}
      </>
    ),
  };
}
