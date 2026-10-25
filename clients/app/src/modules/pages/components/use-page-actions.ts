import { useTranslation } from "react-i18next";

import { useUndoToast } from "~/lib/ui";

import { usePageCalls, useRefreshPages } from "../api/queries";
import type { NewPage, PagePlace, PageRole, PageSummary, PageView } from "../types";

/** Where a page goes. */
export interface Destination {
  parent: string | null;
  place: PagePlace;
  position: string;
}

/**
 * A member's changes to pages: make, move, delete, restore, share. Each
 * shows an Undo (except deleting for good, which the screen confirms first),
 * and a failure is said in a sentence that stays until Retry or dismiss.
 */
export function usePageActions(workspaceId: string) {
  const { t } = useTranslation("pages");
  const calls = usePageCalls(workspaceId);
  const undo = useUndoToast();
  const refresh = useRefreshPages(workspaceId);

  const title = (page: { title: string }) => (page.title === "" ? t("untitled") : page.title);

  /** Run `work`; on failure say so and offer to run it again. */
  const attempt = async <TResult>(
    label: string,
    work: () => Promise<TResult>,
  ): Promise<TResult | undefined> => {
    try {
      const result = await work();
      refresh();
      return result;
    } catch {
      refresh();
      undo.fail({
        label,
        retry: () => {
          void attempt(label, work);
        },
      });
      return undefined;
    }
  };

  return {
    calls,
    /** Make a page; it opens in the caller. Undo puts it in the trash. */
    create: async (body: NewPage): Promise<PageView | undefined> => {
      const made = await attempt(t("toast.createFailed"), () => calls.create(body));
      if (made === undefined) {
        return undefined;
      }
      let current: PageSummary = {
        id: made.id,
        parent: made.parent ?? null,
        place: made.place,
        position: made.position,
        title: made.title,
        icon: made.icon ?? null,
        has_children: false,
        role: made.role,
        created_by: made.created_by,
        edited_at: made.edited_at,
        trashed_at: null,
        meta_revision: made.meta_revision,
      };
      undo.push({
        label: t("toast.created"),
        undo: () => {
          void attempt(t("toast.trashFailed", { title: title(current) }), () => calls.trash(current)).then(
            (trashed) => {
              if (trashed !== undefined) {
                current = trashed;
              }
            },
          );
        },
        redo: () => {
          void attempt(t("toast.restoreFailed", { title: title(current) }), () =>
            calls.restore(current),
          );
        },
      });
      return made;
    },
    /** Put a page and its sub-pages in the trash, with an Undo. */
    trash: async (page: PageSummary): Promise<boolean> => {
      const trashed = await attempt(t("toast.trashFailed", { title: title(page) }), () =>
        calls.trash(page),
      );
      if (trashed === undefined) {
        return false;
      }
      undo.push({
        label: t("toast.trashed", { title: title(page) }),
        undo: () => {
          void attempt(t("toast.restoreFailed", { title: title(page) }), () => calls.restore(trashed));
        },
        redo: () => {
          void attempt(t("toast.trashFailed", { title: title(page) }), () => calls.trash(trashed));
        },
      });
      return true;
    },
    restore: async (page: PageSummary): Promise<void> => {
      const back = await attempt(t("toast.restoreFailed", { title: title(page) }), () =>
        calls.restore(page),
      );
      if (back !== undefined) {
        undo.push({
          label: t("toast.restored", { title: title(page) }),
          undo: () => {
            void attempt(t("toast.trashFailed", { title: title(page) }), () => calls.trash(back));
          },
        });
      }
    },
    /** Delete for good; the screen has asked first. */
    purge: async (page: PageSummary): Promise<void> => {
      const done = await attempt(t("toast.purgeFailed", { title: title(page) }), async () => {
        await calls.purge(page);
        return true;
      });
      if (done === true) {
        undo.notify(t("toast.purged", { title: title(page) }));
      }
    },
    /** Move or reorder a page; Undo moves it back where it was. */
    move: async (page: PageSummary, to: Destination): Promise<void> => {
      const was: Destination = {
        parent: page.parent ?? null,
        place: page.place,
        position: page.position,
      };
      const moved = await attempt(t("toast.moveFailed", { title: title(page) }), () =>
        calls.move(page, to),
      );
      if (moved === undefined) {
        return;
      }
      let current = moved;
      undo.push({
        label: t("toast.moved", { title: title(page) }),
        undo: () => {
          void attempt(t("toast.moveFailed", { title: title(page) }), () =>
            calls.move(current, was),
          ).then((back) => {
            if (back !== undefined) {
              current = back;
            }
          });
        },
        redo: () => {
          void attempt(t("toast.moveFailed", { title: title(page) }), () =>
            calls.move(current, to),
          ).then((again) => {
            if (again !== undefined) {
              current = again;
            }
          });
        },
      });
    },
    /** Share with a member (or change their role); Undo goes back to what it was. */
    share: async (
      page: PageSummary | PageView,
      member: string,
      role: PageRole,
      before: PageRole | undefined,
      name: string,
    ): Promise<void> => {
      const granted = await attempt(t("toast.shareFailed", { name }), () =>
        calls.share(page.id, member, role),
      );
      if (granted === undefined) {
        return;
      }
      undo.push({
        label: t("toast.shared", { name }),
        undo: () => {
          void attempt(t("toast.shareFailed", { name }), () =>
            before === undefined
              ? calls.unshare(page.id, member)
              : calls.share(page.id, member, before),
          );
        },
        redo: () => {
          void attempt(t("toast.shareFailed", { name }), () => calls.share(page.id, member, role));
        },
      });
    },
    /** Stop sharing with a member; Undo shares again with the role they had. */
    unshare: async (
      page: PageSummary | PageView,
      member: string,
      role: PageRole,
      name: string,
    ): Promise<void> => {
      const done = await attempt(t("toast.unshareFailed", { name }), async () => {
        await calls.unshare(page.id, member);
        return true;
      });
      if (done !== true) {
        return;
      }
      undo.push({
        label: t("toast.unshared", { name }),
        undo: () => {
          void attempt(t("toast.shareFailed", { name }), () => calls.share(page.id, member, role));
        },
        redo: () => {
          void attempt(t("toast.unshareFailed", { name }), async () => {
            await calls.unshare(page.id, member);
            return true;
          });
        },
      });
    },
    /** Rename or re-icon a page (the title field settles first). */
    rename: async (
      page: PageSummary,
      change: { title: string; icon: string | null },
    ): Promise<PageSummary | undefined> =>
      attempt(t("toast.renameFailed"), () => calls.update(page, change)),
  };
}
