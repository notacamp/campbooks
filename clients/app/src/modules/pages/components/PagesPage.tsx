import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState, type DragEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Button,
  EmptyState,
  Heading,
  ListPane,
  PageTree,
  PageTreeGroup,
  PageTreeRow,
  PaneHeader,
  ReaderPane,
  SkeletonList,
  Spacer,
} from "~/lib/ui";
import { useMembers } from "~/modules/workspaces";

import { usePages } from "../api/queries";
import {
  buildSections,
  byPosition,
  dropOn,
  isWithin,
  rowsOf,
  zoneOf,
  type TreeNode,
} from "../model";
import { iconOf, type PagePlace, type PageSummary } from "../types";

import { PageScreen } from "./PageScreen";
import { TrashList } from "./TrashList";
import { usePageActions } from "./use-page-actions";

export interface PagesPageProps {
  workspaceId: string;
  /** The page open beside the tree. */
  pageId?: string | undefined;
  /** `?new=1`: make a page and open it (`n p`). */
  creating?: boolean | undefined;
}

type Zone = "before" | "inside" | "after";

/**
 * Pages: the tree (Workspace, Private, Shared with you, the trash) and the
 * open page beside it from `lg`, as its own screen below. Pages are dragged
 * to reorder and nest; every change has an Undo.
 */
export function PagesPage({ workspaceId, pageId, creating = false }: PagesPageProps) {
  const { t } = useTranslation("pages");
  const navigate = useNavigate();
  const pages = usePages(workspaceId);
  const members = useMembers(workspaceId).data ?? [];
  const me = members.find((member) => member.you)?.user_id;
  const actions = usePageActions(workspaceId);
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const [showTrash, setShowTrash] = useState(false);
  const [dragging, setDragging] = useState<string | undefined>();
  const [over, setOver] = useState<{ id: string; zone: Zone } | undefined>();

  const all = pages.data ?? [];
  const sections = buildSections(all, me);
  const base = `/w/${workspaceId}/pages`;
  const go = (id?: string) => {
    void navigate({ to: id === undefined ? base : `${base}/${id}` });
  };

  const make = async (parent?: string, place: PagePlace = "private") => {
    const made = await actions.create(parent === undefined ? { place } : { parent });
    if (made !== undefined) {
      if (parent !== undefined) {
        setOpen((current) => new Set([...current, parent]));
      }
      go(made.id);
    }
  };

  // `?new=1` (from `n p` elsewhere): a new private page, made once.
  const latestMake = useRef(make);
  useEffect(() => {
    latestMake.current = make;
  });
  const started = useRef(false);
  useEffect(() => {
    if (creating && !started.current) {
      started.current = true;
      void latestMake.current();
    }
    if (!creating) {
      started.current = false;
    }
  }, [creating]);
  useShortcut("pages.create", () => {
    void make();
  });

  const toggle = (id: string) => {
    setOpen((current) => {
      const next = new Set(current);
      if (!next.delete(id)) {
        next.add(id);
      }
      return next;
    });
  };

  const siblingsOf = (page: PageSummary) =>
    all
      .filter(
        (other) => (other.parent ?? null) === (page.parent ?? null) && other.place === page.place,
      )
      .toSorted(byPosition);
  const childrenOf = (page: PageSummary) =>
    all.filter((other) => other.parent === page.id).toSorted(byPosition);

  const dragOver = (event: DragEvent<HTMLDivElement>, page: PageSummary) => {
    if (dragging === undefined || isWithin(sections, dragging, page.id)) {
      return;
    }
    event.preventDefault();
    const box = event.currentTarget.getBoundingClientRect();
    const zone = zoneOf(event.clientY - box.top, box.height);
    if (over?.id !== page.id || over.zone !== zone) {
      setOver({ id: page.id, zone });
    }
  };
  const drop = (event: DragEvent<HTMLDivElement>, page: PageSummary) => {
    event.preventDefault();
    const moving = all.find((candidate) => candidate.id === dragging);
    const zone = over?.zone ?? "inside";
    setDragging(undefined);
    setOver(undefined);
    if (moving === undefined || isWithin(sections, moving.id, page.id)) {
      return;
    }
    const destination = dropOn(page, zone, siblingsOf(page), childrenOf(page), moving.id);
    if (zone === "inside") {
      setOpen((current) => new Set([...current, page.id]));
    }
    void actions.move(moving, destination);
  };

  const rows = (nodes: readonly TreeNode[]) =>
    rowsOf(nodes, open).map(({ page, depth, expanded }) => (
      <PageTreeRow
        key={page.id}
        label={page.title === "" ? t("untitled") : page.title}
        icon={iconOf(page.icon)}
        depth={depth}
        expanded={expanded}
        current={page.id === pageId}
        toggleLabel={t(expanded === true ? "tree.hideSubPages" : "tree.showSubPages", {
          title: page.title === "" ? t("untitled") : page.title,
        })}
        addLabel={t("tree.addSubPage", { title: page.title === "" ? t("untitled") : page.title })}
        onOpen={() => {
          go(page.id);
        }}
        onToggle={() => {
          toggle(page.id);
        }}
        {...(page.role === "reader"
          ? {}
          : {
              onAdd: () => {
                void make(page.id);
              },
            })}
        draggable={page.role !== "reader"}
        dragging={dragging === page.id}
        dropZone={over?.id === page.id ? over.zone : undefined}
        onDragStart={(event) => {
          event.dataTransfer.effectAllowed = "move";
          event.dataTransfer.setData("text/plain", page.id);
          setDragging(page.id);
        }}
        onDragOver={(event) => {
          dragOver(event, page);
        }}
        onDragLeave={() => {
          setOver(undefined);
        }}
        onDrop={(event) => {
          drop(event, page);
        }}
        onDragEnd={() => {
          setDragging(undefined);
          setOver(undefined);
        }}
      />
    ));

  const nothing = pages.isSuccess && all.length === 0;

  return (
    <>
      <ListPane label={t("tree.label")} narrow={pageId === undefined ? "shown" : "hidden"}>
        <PaneHeader>
          <Heading level={1} size="pane">
            {t("title")}
          </Heading>
          <Spacer />
          <Button
            variant="ghost"
            icon="trash"
            aria-pressed={showTrash}
            onClick={() => {
              setShowTrash((shown) => !shown);
            }}
          >
            {t("tree.trash")}
          </Button>
          <Button
            icon="plus"
            shortcut="pages.create"
            onClick={() => {
              void make();
            }}
          >
            {t("tree.newPage")}
          </Button>
        </PaneHeader>
        {pages.isPending ? <SkeletonList rows={5} /> : null}
        {pages.isError ? (
          <EmptyState
            title={t("error.title")}
            visor="thinking"
            action={
              <Button
                onClick={() => {
                  void pages.refetch();
                }}
              >
                {t("error.retry")}
              </Button>
            }
          >
            {t("error.body")}
          </EmptyState>
        ) : null}
        {showTrash ? (
          <TrashList workspaceId={workspaceId} actions={actions} />
        ) : nothing ? (
          <EmptyState
            title={t("empty.title")}
            visor="happy"
            visorSize="lg"
            action={
              <Button
                variant="primary"
                icon="plus"
                onClick={() => {
                  void make();
                }}
              >
                {t("empty.add")}
              </Button>
            }
          >
            {t("empty.body")}
          </EmptyState>
        ) : pages.isSuccess ? (
          <PageTree label={t("tree.label")}>
            <PageTreeGroup
              name={t("tree.workspace")}
              action={
                <Button
                  variant="ghost"
                  icon="plus"
                  onClick={() => {
                    void make(undefined, "workspace");
                  }}
                >
                  {t("tree.newPage")}
                </Button>
              }
              empty={t("tree.workspaceEmpty")}
            >
              {sections.workspace.length === 0 ? undefined : rows(sections.workspace)}
            </PageTreeGroup>
            <PageTreeGroup name={t("tree.private")} empty={t("tree.privateEmpty")}>
              {sections.private.length === 0 ? undefined : rows(sections.private)}
            </PageTreeGroup>
            {sections.shared.length === 0 ? null : (
              <PageTreeGroup name={t("tree.shared")}>{rows(sections.shared)}</PageTreeGroup>
            )}
          </PageTree>
        ) : null}
      </ListPane>
      <ReaderPane label={t("screen.label")} narrow={pageId === undefined ? "hidden" : "shown"}>
        {pageId === undefined ? (
          <div className="flex flex-col items-center gap-2 p-6">
            <EmptyState title={t("screen.emptyTitle")} visor="idle" visorSize="lg">
              {t("screen.emptyBody")}
            </EmptyState>
          </div>
        ) : (
          <PageScreen
            workspaceId={workspaceId}
            pageId={pageId}
            pages={all}
            actions={actions}
            onBack={() => {
              go();
            }}
          />
        )}
      </ReaderPane>
    </>
  );
}
