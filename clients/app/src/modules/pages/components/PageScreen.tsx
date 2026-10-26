import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import {
  Button,
  ChipLink,
  ChipRow,
  EmptyState,
  Heading,
  IconButton,
  Popover,
  PopoverSection,
  ReaderBody,
  SkeletonList,
  Spacer,
  StatusText,
  Text,
  TitleField,
} from "~/lib/ui";

import { useNotionLink } from "../api/notion";
import { useBacklinks, usePage, useTablesAvailable } from "../api/queries";
import { PageEditor, type TableHelpers } from "../editor/PageEditor";
import { useTableCalls } from "../tables/api";
import { RowHeader } from "../tables/components/RowHeader";
import { TableView } from "../tables/components/TableView";
import type { SaveState } from "../editor/sync";
import { usePageDocument } from "../editor/use-page-document";
import type { PageSummary, PageView } from "../types";

import { IconPicker } from "./IconPicker";
import { NotionSync } from "./NotionSync";
import { ShareMenu } from "./ShareMenu";
import { useNotionActions } from "./use-notion-actions";
import type { usePageActions } from "./use-page-actions";

/** How long typing in the title is gathered before it is saved, in ms. */
const TITLE_PAUSE_MS = 600;

export interface PageScreenProps {
  workspaceId: string;
  pageId: string;
  pages: readonly PageSummary[];
  actions: ReturnType<typeof usePageActions>;
  /** Back to the tree (on phones). */
  onBack: () => void;
}

/** The page open in the reader pane: loading, not found, or the page. */
export function PageScreen({ workspaceId, pageId, pages, actions, onBack }: PageScreenProps) {
  const { t } = useTranslation("pages");
  const page = usePage(workspaceId, pageId);
  if (page.isPending) {
    return (
      <ReaderBody>
        <SkeletonList rows={5} />
      </ReaderBody>
    );
  }
  if (page.isError) {
    return (
      <EmptyState
        title={t("screen.missingTitle")}
        visor="unsure"
        action={<Button onClick={onBack}>{t("screen.back")}</Button>}
      >
        {t("screen.missingBody")}
      </EmptyState>
    );
  }
  return (
    <OpenPage
      // A page opens with its own document and its own title field.
      key={page.data.id}
      workspaceId={workspaceId}
      page={page.data}
      pages={pages}
      actions={actions}
      onBack={onBack}
    />
  );
}

const SAVE_TONE = {
  saved: "ok",
  saving: "waiting",
  retrying: "problem",
  rejected: "problem",
} as const;

interface OpenPageProps extends Omit<PageScreenProps, "pageId"> {
  page: PageView;
}

function OpenPage({ workspaceId, page, pages, actions, onBack }: OpenPageProps) {
  const { t } = useTranslation("pages");
  const navigate = useNavigate();
  const trashed = page.trashed_at !== null && page.trashed_at !== undefined;
  const editable = !trashed && page.role !== "reader";
  const { doc, state } = usePageDocument({
    pageId: page.id,
    update: page.document.update,
    stateVector: page.document.state_vector,
    revision: page.revision,
    calls: actions.calls,
  });

  const notionLink = useNotionLink(workspaceId, page.id);
  const notion = useNotionActions(workspaceId);
  const article = useRef<HTMLElement>(null);
  const showConflict = () => {
    article.current
      ?.querySelector('[data-type="conflict"]')
      ?.scrollIntoView({ behavior: "smooth", block: "center" });
  };

  const [title, setTitle] = useState(page.title);
  const [icon, setIcon] = useState<string | null>(page.icon ?? null);
  const meta = useRef(page.meta_revision);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const save = (next: { title: string; icon: string | null }) => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      const seen = pages.find((candidate) => candidate.id === page.id);
      void actions
        .rename({ ...(seen ?? summaryOf(page)), meta_revision: meta.current }, next)
        .then((saved) => {
          if (saved !== undefined) {
            meta.current = saved.meta_revision;
          }
        });
    }, TITLE_PAUSE_MS);
  };
  useEffect(
    () => () => {
      clearTimeout(timer.current);
    },
    [],
  );

  const open = (href: string) => {
    void navigate({ to: href });
  };
  const link = (href: string) => ({
    href,
    onClick: (event: MouseEvent<HTMLElement>) => {
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) {
        return;
      }
      event.preventDefault();
      open(href);
    },
  });
  const base = `/w/${workspaceId}/pages`;
  const section = t(page.place === "workspace" ? "tree.workspace" : "tree.private");
  const backlinks = useBacklinks(workspaceId, "pages.page", page.id).data ?? [];
  const summary = pages.find((candidate) => candidate.id === page.id) ?? summaryOf(page);
  const tablesOn = useTablesAvailable(workspaceId);
  const tableCalls = useTableCalls(workspaceId);
  const tableHelpers: TableHelpers = {
    create: async () => {
      const made = await tableCalls.createTable({ parent: page.id });
      return { table: made.page.id, view: made.views[0]?.id ?? "" };
    },
    discard: async (table) => {
      await tableCalls.trashPage(table);
    },
    firstView: async (table) => (await tableCalls.getTable(table)).views[0]?.id ?? "",
    isTable: (id) => pages.some((candidate) => candidate.id === id && candidate.kind === "table"),
  };
  const isTable = page.kind === "table";
  const isRow = page.kind === "row" && page.parent !== null && page.parent !== undefined;

  return (
    <>
      <div className="flex flex-wrap items-center gap-2 p-3">
        <IconButton icon="chevron-left" label={t("screen.back")} size="lg" onClick={onBack} />
        <nav
          aria-label={t("screen.trail")}
          className="flex w-full min-w-0 items-center gap-1 sm:w-auto sm:flex-1"
        >
          <Text as="span" tone="tertiary" size="sm">
            {section}
          </Text>
          {page.ancestors.map((crumb) => (
            <span key={crumb.id} className="flex min-w-0 items-center gap-1">
              <Text as="span" tone="tertiary" size="sm">
                /
              </Text>
              <a {...link(`${base}/${crumb.id}`)}>
                <Text as="span" tone="secondary" size="sm" truncate>
                  {crumb.title === "" ? t("untitled") : crumb.title}
                </Text>
              </a>
            </span>
          ))}
        </nav>
        <SaveLabel state={state} />
        <ShareMenu workspaceId={workspaceId} page={page} actions={actions} />
        <Popover
          label={t("screen.more")}
          renderTrigger={(trigger) => (
            <IconButton {...trigger} icon="more" label={t("screen.more")} size="lg" />
          )}
        >
          {(close) => (
            <PopoverSection>
              <Button
                variant="ghost"
                icon="trash"
                disabled={page.role !== "manager" || trashed}
                onClick={() => {
                  close();
                  void actions.trash(summary).then((done) => {
                    if (done) {
                      onBack();
                    }
                  });
                }}
              >
                {t("screen.trash")}
              </Button>
            </PopoverSection>
          )}
        </Popover>
      </div>
      <ReaderBody>
        <article
          ref={article}
          className={
            isTable
              ? "mx-auto flex w-full flex-col gap-3 p-4"
              : "mx-auto flex w-full max-w-measure flex-col gap-3 p-4"
          }
        >
          {trashed ? <Text tone="secondary">{t("screen.inTrash")}</Text> : null}
          <div className="flex items-start gap-2">
            {editable ? (
              <IconPicker
                value={icon}
                onChange={(next) => {
                  setIcon(next);
                  save({ title, icon: next });
                }}
              />
            ) : null}
            <TitleField
              label={t("screen.titleLabel")}
              placeholder={t("untitled")}
              value={title}
              readOnly={!editable}
              onChange={(event) => {
                setTitle(event.target.value);
                save({ title: event.target.value, icon });
              }}
            />
          </div>
          <Text tone="tertiary" size="sm">
            {t("screen.edited", {
              when: new Intl.DateTimeFormat(undefined, {
                dateStyle: "medium",
                timeStyle: "short",
              }).format(new Date(page.edited_at)),
            })}
          </Text>
          {notionLink === undefined ? null : (
            <NotionSync
              link={notionLink}
              canStop={editable}
              onStop={() => notion.stop(page.id)}
              onShowConflict={showConflict}
            />
          )}
          {isRow ? (
            <RowHeader
              workspaceId={workspaceId}
              rowId={page.id}
              tableId={page.parent ?? ""}
              pages={pages}
              pageActions={actions}
              open={open}
            />
          ) : null}
          {isTable ? (
            <TableView
              workspaceId={workspaceId}
              tableId={page.id}
              pages={pages}
              pageActions={actions}
              open={open}
            />
          ) : (
            <PageEditor
              environment={{ workspaceId, pageId: page.id, open }}
              doc={doc}
              editable={editable}
              pages={pages}
              subPages={page.kind === "page"}
              tables={page.kind === "page" && tablesOn ? tableHelpers : undefined}
              upload={(file) => actions.calls.upload(page.id, file)}
              createSubPage={async () => {
                const made = await actions.create({ parent: page.id });
                if (made === undefined) {
                  throw new Error("sub-page not made");
                }
                return made.id;
              }}
            />
          )}
          {backlinks.length === 0 ? null : (
            <section className="flex flex-col gap-2 pt-6">
              <Heading level={2} size="pane">
                {t("backlinks.title", { count: backlinks.length })}
              </Heading>
              <ChipRow label={t("backlinks.title", { count: backlinks.length })}>
                {backlinks.map((from) => (
                  <ChipLink key={from.page} {...link(`${base}/${from.page}`)}>
                    {from.title === "" ? t("untitled") : from.title}
                  </ChipLink>
                ))}
              </ChipRow>
            </section>
          )}
          <Spacer />
        </article>
      </ReaderBody>
    </>
  );
}

function SaveLabel({ state }: { state: SaveState }) {
  const { t } = useTranslation("pages");
  return (
    <span role="status">
      <StatusText tone={SAVE_TONE[state]}>{t(`save.${state}`)}</StatusText>
    </span>
  );
}

/** A page's tree entry, from the page itself, for a page the list has not got yet. */
function summaryOf(page: PageView): PageSummary {
  return {
    id: page.id,
    parent: page.parent ?? null,
    place: page.place,
    position: page.position,
    title: page.title,
    icon: page.icon ?? null,
    has_children: false,
    role: page.role,
    created_by: page.created_by,
    edited_at: page.edited_at,
    trashed_at: page.trashed_at ?? null,
    meta_revision: page.meta_revision,
    kind: page.kind,
  };
}
