import { useNavigate } from "@tanstack/react-router";
import { useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

import { conversationRef, useScoutChat } from "~/lib/scout-chat";
import {
  Banner,
  Button,
  Checkbox,
  ChoiceCards,
  Dialog,
  Picker,
  Segmented,
  Text,
  TextField,
  useUndoToast,
  type PickerItem,
} from "~/lib/ui";
import { useMembers } from "~/modules/workspaces";

import { mailScoutCalls, readPage, usePageSharing } from "../api/mail-scout";
import { useBacklinks, usePageCalls, usePages, useRefreshPages } from "../api/queries";
import type { PageSummary } from "../types";

import { threadBlock } from "./blocks";
import { buildCopy } from "./copy";
import { readersOf, type Readers } from "./readers";
import { tooLarge } from "./refusals";
import { saveToPage, SaveRefused, takeBack, type Saved, type SaveCalls } from "./save";
import type { SaveThread } from "./thread";
import { fold, readersLine, refusalText, type Translate } from "./wording";

/** A clean-up that fails has nothing more to take back. */
const noop = (): void => {
  // Nothing more to take back.
};

type Where = "existing" | "new";
type What = "live" | "copy" | "summary";

export interface SaveToPageDialogProps {
  workspaceId: string;
  thread: SaveThread;
  open: boolean;
  onClose: () => void;
}

/**
 * "Save to page" (campbooks-cloud#364): a thread goes onto a page the member
 * can write, or onto a new private page. A live block that always shows the
 * newest message, a copy of the messages (saying who will read the page) or
 * Scout's summary (which hands over to Scout's chat, whose card is the
 * preview). Attachments can come along. Saved, a toast offers Open page and
 * Undo; a refusal is said by name and nothing is half-added.
 */
export function SaveToPageDialog({ workspaceId, thread, open, onClose }: SaveToPageDialogProps) {
  const { t: typed, i18n } = useTranslation("pages");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const undo = useUndoToast();
  const scout = useScoutChat();
  const pageCalls = usePageCalls(workspaceId);
  const refresh = useRefreshPages(workspaceId);
  const members = useMembers(workspaceId).data ?? [];
  const pages = usePages(workspaceId);
  const linked = useBacklinks(workspaceId, "conversations.conversation", thread.id);

  const [where, setWhere] = useState<Where>("existing");
  const [what, setWhat] = useState<What>("live");
  const [query, setQuery] = useState("");
  const [chosen, setChosen] = useState<string | undefined>();
  const [title, setTitle] = useState(thread.subject);
  const [withFiles, setWithFiles] = useState(false);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState<string | undefined>();

  const files = thread.messages.flatMap((message) =>
    message.attachments.map((file) => ({ ...file, message: message.id })),
  );
  const subject = thread.subject === "" ? t("saveToPage.noSubject") : thread.subject;

  // Pages the member can write, the ones already linking this thread first.
  const linkedIds = new Set((linked.data ?? []).map((link) => link.page));
  const writable = (pages.data ?? [])
    .filter((page) => page.kind === "page" && page.role !== "reader")
    .toSorted(
      (a, b) =>
        Number(linkedIds.has(b.id)) - Number(linkedIds.has(a.id)) ||
        b.edited_at.localeCompare(a.edited_at),
    );
  const wanted = fold(query.trim());
  const matches = writable.filter((page) => fold(page.title).includes(wanted));
  const target: PageSummary | undefined = writable.find((page) => page.id === chosen);
  const items: PickerItem[] = matches.map((page) => ({
    id: page.id,
    text: page.title === "" ? t("untitled") : page.title,
    label: page.title === "" ? t("untitled") : page.title,
    selected: page.id === chosen,
    ...(linkedIds.has(page.id) ? { hint: t("saveToPage.linked") } : {}),
  }));

  const sharing = usePageSharing(
    workspaceId,
    where === "existing" && what === "copy" ? chosen : undefined,
  );
  let readers: Readers | undefined;
  if (what === "copy") {
    if (where === "new") {
      readers = { everyone: false, names: [], others: 0 };
    } else if (sharing.data !== undefined) {
      readers = readersOf(sharing.data, members);
    }
  }
  const toNotion = what === "copy" && where === "existing" && sharing.data?.notion === true;

  const big = withFiles && what !== "summary" ? tooLarge(files) : undefined;
  const ready =
    !busy && big === undefined && (where === "new" ? title.trim() !== "" : target !== undefined);

  const calls: SaveCalls & { trash: (pageId: string) => Promise<void> } = {
    read: (pageId) => readPage(workspaceId, pageId),
    sync: pageCalls.sync,
    attach: mailScoutCalls(workspaceId).attach,
    detach: mailScoutCalls(workspaceId).detach,
    trash: mailScoutCalls(workspaceId).trash,
  };

  const finish = (pageId: string, pageTitle: string, made: boolean, saved: Saved) => {
    refresh();
    undo.push({
      label: t("saveToPage.saved", { page: pageTitle === "" ? t("untitled") : pageTitle }),
      undo: () => {
        void (async () => {
          try {
            await takeBack(calls, saved);
            if (made) {
              await calls.trash(pageId);
            }
            refresh();
          } catch {
            undo.fail({ label: t("saveToPage.undoFailed") });
          }
        })();
      },
      open: {
        label: t("saveToPage.open"),
        run: () => {
          void navigate({ to: `/w/${workspaceId}/pages/${pageId}` });
        },
      },
    });
  };

  const handOver = () => {
    const page = where === "new" ? title.trim() : (target?.title ?? "");
    onClose();
    scout?.(conversationRef(thread.id), {
      ask:
        where === "new"
          ? t("saveToPage.askNew", { title: page })
          : t("saveToPage.askExisting", { page: page === "" ? t("untitled") : page }),
    });
  };

  const submit = async () => {
    if (!ready) {
      return;
    }
    if (what === "summary") {
      handOver();
      return;
    }
    setBusy(true);
    setProblem(undefined);
    let made: string | undefined;
    try {
      let pageId = target?.id;
      let pageTitle = target?.title ?? "";
      if (where === "new") {
        const page = await pageCalls.create({
          title: title.trim(),
          place: "private",
          parent: null,
        });
        made = page.id;
        pageId = page.id;
        pageTitle = page.title;
      }
      if (pageId === undefined) {
        return;
      }
      const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });
      const blocks =
        what === "live"
          ? [threadBlock(thread.id)]
          : buildCopy(thread.messages, (iso) => date.format(new Date(iso)));
      const saved = await saveToPage(calls, {
        pageId,
        conversation: thread.id,
        blocks,
        attachments: withFiles
          ? thread.messages
              .filter((message) => message.attachments.length > 0)
              .map((message) => ({ message: message.id, files: message.attachments }))
          : [],
      });
      finish(pageId, pageTitle, made !== undefined, saved);
      onClose();
    } catch (error) {
      if (made !== undefined) {
        // A page made for this alone goes back to the trash with it.
        await calls.trash(made).catch(noop);
        refresh();
      }
      setProblem(
        error instanceof SaveRefused
          ? refusalText(t, error.refusal)
          : t("saveToPage.refused.failed"),
      );
    } finally {
      setBusy(false);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void submit();
    }
  };

  const options = [
    {
      value: "live" as const,
      title: t("saveToPage.live.title"),
      description: t("saveToPage.live.body"),
    },
    {
      value: "copy" as const,
      title: t("saveToPage.copy.title"),
      description: t("saveToPage.copy.body"),
    },
    ...(scout === undefined
      ? []
      : [
          {
            value: "summary" as const,
            title: t("saveToPage.summary.title"),
            description: t("saveToPage.summary.body"),
          },
        ]),
  ];

  return (
    <Dialog
      open={open}
      onClose={() => {
        if (!busy) {
          onClose();
        }
      }}
      title={t("saveToPage.title", { subject })}
      footer={
        <>
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {t("saveToPage.cancel")}
          </Button>
          <Button
            variant="primary"
            keys="mod+enter"
            disabled={!ready}
            onClick={() => {
              void submit();
            }}
          >
            {what === "summary" ? t("saveToPage.preview") : t("saveToPage.save")}
          </Button>
        </>
      }
    >
      <div role="presentation" className="flex flex-col gap-4" onKeyDown={onKeyDown}>
        <Segmented
          label={t("saveToPage.where")}
          value={where}
          onChange={setWhere}
          options={[
            { value: "existing", label: t("saveToPage.existing") },
            { value: "new", label: t("saveToPage.new") },
          ]}
        />
        {where === "existing" ? (
          <Picker
            label={t("saveToPage.pages")}
            searchLabel={t("saveToPage.search")}
            placeholder={t("saveToPage.search")}
            query={query}
            onQuery={setQuery}
            items={items}
            onPick={setChosen}
            empty={t("saveToPage.none")}
            {...(pages.isPending ? { loading: t("saveToPage.loading") } : {})}
          />
        ) : (
          <div className="flex flex-col gap-2">
            <TextField
              label={t("saveToPage.newTitle")}
              autoComplete="off"
              value={title}
              onChange={(event) => {
                setTitle(event.target.value);
              }}
            />
            <Text tone="tertiary" size="sm">
              {t("saveToPage.newIn")}
            </Text>
          </div>
        )}
        <ChoiceCards
          label={t("saveToPage.what")}
          options={options}
          value={what}
          onChange={setWhat}
        />
        {what === "copy" ? (
          <div className="flex flex-col gap-1" aria-live="polite">
            {readers === undefined ? null : <Text size="sm">{readersLine(t, readers)}</Text>}
            {toNotion ? (
              <Text size="sm" tone="secondary">
                {t("saveToPage.readers.notion")}
              </Text>
            ) : null}
          </div>
        ) : null}
        {files.length > 0 && what !== "summary" ? (
          <Checkbox
            label={t("saveToPage.files", { count: files.length })}
            checked={withFiles}
            onChange={(event) => {
              setWithFiles(event.target.checked);
            }}
          />
        ) : null}
        {big === undefined ? null : (
          <Banner
            tone="danger"
            role="alert"
            title={t("saveToPage.refused.tooLarge", { name: big.name })}
          />
        )}
        {problem === undefined ? null : <Banner tone="danger" role="alert" title={problem} />}
      </div>
    </Dialog>
  );
}
