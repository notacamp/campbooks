import type { InfiniteData, UseInfiniteQueryResult } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useEffect, useState, type MouseEvent, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button, ButtonLink, EmptyState, SettingsPage, SkeletonList, TextField } from "~/lib/ui";

/** `value`, once it has stopped changing for `ms`. */
export function useDebounced<T>(value: T, ms = 250): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => {
      setSettled(value);
    }, ms);
    return () => {
      clearTimeout(timer);
    };
  }, [value, ms]);
  return settled;
}

/** Props of a link the router follows in place (a plain click) or the browser opens (a modified one). */
export function useRouterLink() {
  const navigate = useNavigate();
  return (href: string) => ({
    href,
    onClick: (event: MouseEvent<HTMLElement>) => {
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) {
        return;
      }
      event.preventDefault();
      void navigate({ to: href });
    },
  });
}

/** Dates as the reader's language writes them ("13 Oct 2026"). */
export function useDateFormat() {
  const { i18n } = useTranslation();
  const format = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });
  return (iso: string) => format.format(new Date(iso));
}

export interface AdminListPanelProps<TPage> {
  title: string;
  lead: string;
  searchLabel: string;
  searchPlaceholder: string;
  /** What is typed in the search field. */
  search: string;
  onSearch: (text: string) => void;
  /** The pages for the settled search text. */
  pages: UseInfiniteQueryResult<InfiniteData<TPage>>;
  /** The rows of every loaded page, as a `List`; `undefined` when there are none. */
  rows: ReactNode | undefined;
  loadingLabel: string;
  /** Shown when nothing matches (`search` is the settled text, or empty). */
  emptyTitle: (search: string) => string;
  errorTitle: string;
  errorLead: string;
}

/**
 * The shape of an admin list (People, Workspaces): a title, a lead, a
 * search field, the rows, and "Show more" while the API has another page.
 */
export function AdminListPanel<TPage>({
  title,
  lead,
  searchLabel,
  searchPlaceholder,
  search,
  onSearch,
  pages,
  rows,
  loadingLabel,
  emptyTitle,
  errorTitle,
  errorLead,
}: AdminListPanelProps<TPage>) {
  const { t } = useTranslation("workspaces");
  const retry = () => {
    void pages.refetch();
  };
  return (
    <SettingsPage label={title} title={title} lead={lead}>
      <form
        role="search"
        onSubmit={(event) => {
          event.preventDefault();
        }}
      >
        <TextField
          type="search"
          autoComplete="off"
          label={searchLabel}
          placeholder={searchPlaceholder}
          value={search}
          onChange={(event) => {
            onSearch(event.target.value);
          }}
        />
      </form>
      {pages.isPending ? (
        <div role="status" aria-label={loadingLabel}>
          <SkeletonList rows={4} />
        </div>
      ) : pages.isError ? (
        <EmptyState
          title={errorTitle}
          visor="thinking"
          visorSize="lg"
          action={<Button onClick={retry}>{t("admin.retry")}</Button>}
        >
          {errorLead}
        </EmptyState>
      ) : rows === undefined ? (
        <EmptyState title={emptyTitle(search.trim())} visor="happy" visorSize="lg" />
      ) : (
        <>
          {rows}
          {pages.hasNextPage ? (
            <div className="flex">
              <Button
                disabled={pages.isFetchingNextPage}
                onClick={() => {
                  void pages.fetchNextPage();
                }}
              >
                {pages.isFetchingNextPage ? t("admin.loadingMore") : t("admin.more")}
              </Button>
            </div>
          ) : null}
        </>
      )}
    </SettingsPage>
  );
}

export interface AdminDetailFrameProps {
  /** The page's name ("Ana Duarte"), or what is loading. */
  title: string;
  /** Back to the list ("People"). */
  backLabel: string;
  backHref: string;
  children: ReactNode;
}

/** A detail page of the admin section: back to the list, a title, the content. */
export function AdminDetailFrame({ title, backLabel, backHref, children }: AdminDetailFrameProps) {
  const link = useRouterLink();
  return (
    <SettingsPage label={title} title={title}>
      <div className="flex">
        <ButtonLink variant="ghost" icon="chevron-left" {...link(backHref)}>
          {backLabel}
        </ButtonLink>
      </div>
      {children}
    </SettingsPage>
  );
}
