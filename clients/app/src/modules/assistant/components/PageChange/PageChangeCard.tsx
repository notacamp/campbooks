import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { refreshRecords, type RecordRef } from "~/lib/api";
import {
  Button,
  ChatProposal,
  Chip,
  ChipLink,
  ChipRow,
  Clamp,
  NoteField,
  RichDocument,
  Text,
  type ChatProposalState,
} from "~/lib/ui";
import { useConversationsByIds } from "~/modules/conversations";
import { usePageTrail } from "~/modules/pages";

import {
  markdownBytes,
  MARKDOWN_MAX_BYTES,
  mayAdd,
  pageProposalOf,
  readersSentence,
  type PageProposalView,
} from "../../api/page-proposal";
import {
  NotRunnable,
  PAGE_UNDOS,
  refusalReason,
  runnable,
  runRequest,
  settleProposal,
  undoBody,
  withAnswer,
  type Proposal,
  type Settlement,
} from "../../api/proposals";

import { PageMarkdown } from "./markdown";

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

export interface PageChangeCardProps {
  workspaceId: string;
  chatId: string;
  proposal: Proposal;
  /** The card was settled: the chat is read again. */
  onSettled: () => void;
  /** The member is leaving the chat for a page or a thread (a narrow chat closes). */
  onLeave?: (() => void) | undefined;
}

/**
 * A page change Scout proposes (campbooks-cloud#364): a section for the end
 * of a page, or a new private page. The card shows what is being confirmed in
 * full: where it goes, who will read it (never "private" when anyone else
 * can), whether Notion gets it too, and the whole Markdown as the page would
 * show it, folded past about twelve lines with "Show all". "Add to page" is
 * offered only when the card can show all of that; "Edit first" changes the
 * text that is sent; once added the card offers Open page and Undo.
 */
export function PageChangeCard(props: PageChangeCardProps) {
  const { workspaceId, chatId, proposal, onSettled, onLeave } = props;
  const { t: typed } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const view = pageProposalOf(proposal);
  const request = proposal.request ?? undefined;
  const undo = proposal.undo ?? undefined;
  const ready = request === undefined ? undefined : runnable(request);
  const undoReady = undo === undefined ? undefined : runnable(undo, PAGE_UNDOS);
  const [phase, setPhase] = useState<ChatProposalState | undefined>();
  const [reason, setReason] = useState<string | undefined>();
  const [undoFailed, setUndoFailed] = useState(false);
  const [answer, setAnswer] = useState<{ value: unknown } | undefined>();
  const [edited, setEdited] = useState<string | undefined>();
  const [editing, setEditing] = useState(false);
  const acting = useRef(false);
  const trail = usePageTrail(workspaceId, view?.pageId);

  // Without everything to show, or a request this client runs, nothing can be added.
  const shown = view !== undefined && ready !== undefined;
  const state: ChatProposalState =
    phase ?? (proposal.state === "pending" ? (shown ? "pending" : "blocked") : proposal.state);
  const markdown = edited ?? view?.markdown ?? "";

  const settle = async (next: Settlement) => {
    try {
      await settleProposal(workspaceId, chatId, proposal.id, next);
    } catch {
      // The card still says what happened; Scout reads it as unanswered.
    } finally {
      onSettled();
    }
  };

  const add = async () => {
    if (acting.current || request === undefined || ready === undefined || !mayAdd(view, markdown)) {
      return;
    }
    acting.current = true;
    setPhase("working");
    setEditing(false);
    try {
      // The text sent is the text on the card, edited or not.
      const body =
        typeof request.body === "object" && request.body !== null
          ? { ...(request.body as Record<string, unknown>), markdown }
          : { markdown };
      const value = await runRequest(workspaceId, request, proposal.id, body);
      setAnswer({ value });
      setPhase("done");
      void refreshRecords(queryClient, workspaceId, ready.entry.records);
      await settle("done");
    } catch (error) {
      if (error instanceof NotRunnable) {
        setPhase("blocked");
      } else {
        setReason(refusalReason(error));
        setPhase("failed");
        await settle("failed");
      }
    } finally {
      acting.current = false;
    }
  };

  const takeBack = async () => {
    if (acting.current || undo === undefined || answer === undefined) {
      return;
    }
    acting.current = true;
    setUndoFailed(false);
    setPhase("working");
    try {
      const target = withAnswer(undo, answer.value);
      if (target === undefined) {
        throw new NotRunnable();
      }
      await runRequest(
        workspaceId,
        target,
        `${proposal.id}:undo`,
        undoBody(undo, answer.value),
        PAGE_UNDOS,
      );
      setPhase("undone");
      void refreshRecords(queryClient, workspaceId, undoReady?.entry.records ?? []);
      await settle("undone");
    } catch {
      setUndoFailed(true);
      setPhase("done");
    } finally {
      acting.current = false;
    }
  };

  const discard = async () => {
    if (acting.current) {
      return;
    }
    setPhase("declined");
    await settle("declined");
  };

  /** The page that was changed or made. */
  const addedId = (): string | undefined => {
    if (view?.kind === "create") {
      const value = answer?.value;
      return typeof value === "object" &&
        value !== null &&
        "id" in value &&
        typeof value.id === "string"
        ? value.id
        : undefined;
    }
    return ready?.params["page_id"] ?? view?.pageId;
  };
  const openPage = () => {
    const id = addedId();
    if (id !== undefined) {
      onLeave?.();
      void navigate({ to: `/w/${workspaceId}/pages/${id}` });
    }
  };

  const title =
    view === undefined
      ? t("chat.proposal.page.unknown")
      : view.kind === "append"
        ? t("chat.proposal.page.addTo", { title: pageName(t, view.title) })
        : view.parent === undefined
          ? t("chat.proposal.page.newPage")
          : t("chat.proposal.page.newUnder", { parent: view.parent });
  const about =
    view === undefined
      ? undefined
      : view.kind === "append"
        ? trail.length > 1
          ? trail.slice(0, -1).join(" / ")
          : undefined
        : pageName(t, view.title);

  const status = {
    pending: undefined,
    working: t("chat.proposal.working"),
    done: undoFailed ? t("chat.proposal.undoFailed") : t("chat.proposal.page.added"),
    undone: t("chat.proposal.undone"),
    declined: t("chat.proposal.declined"),
    failed:
      reason === undefined
        ? t("chat.proposal.failed")
        : t("chat.proposal.failedBecause", { reason }),
    blocked: view === undefined ? t("chat.proposal.page.cannotShow") : t("chat.proposal.blocked"),
  }[state];

  const stillPending = proposal.state === "pending" && phase === undefined;
  const tooLong = markdownBytes(markdown) > MARKDOWN_MAX_BYTES;
  const actions =
    state === "pending" ? (
      <>
        <Button
          variant="primary"
          disabled={!mayAdd(view, markdown)}
          onClick={() => {
            void add();
          }}
        >
          {t("chat.proposal.page.add")}
        </Button>
        <Button
          onClick={() => {
            if (editing) {
              setEditing(false);
            } else {
              setEdited((current) => current ?? view?.markdown ?? "");
              setEditing(true);
            }
          }}
        >
          {editing ? t("chat.proposal.page.preview") : t("chat.proposal.page.edit")}
        </Button>
        <Button
          onClick={() => {
            void discard();
          }}
        >
          {t("chat.proposal.page.discard")}
        </Button>
      </>
    ) : state === "blocked" && stillPending ? (
      <Button
        onClick={() => {
          void discard();
        }}
      >
        {t("chat.proposal.page.discard")}
      </Button>
    ) : state === "done" && answer !== undefined ? (
      <>
        {addedId() === undefined ? null : (
          <Button onClick={openPage}>{t("chat.proposal.page.open")}</Button>
        )}
        {undoReady === undefined ? null : (
          <Button
            onClick={() => {
              void takeBack();
            }}
          >
            {t("chat.proposal.undo")}
          </Button>
        )}
      </>
    ) : undefined;

  return (
    <ChatProposal
      label={t(
        state === "pending" || state === "working"
          ? "chat.proposal.label"
          : "chat.proposal.labelPast",
      )}
      title={title}
      about={about}
      body={
        view === undefined ? undefined : (
          <PageChangeBody
            workspaceId={workspaceId}
            view={view}
            markdown={markdown}
            editing={editing && state === "pending"}
            tooLong={tooLong}
            onEdit={setEdited}
            onAdd={() => {
              void add();
            }}
            onStopEditing={() => {
              setEditing(false);
            }}
            onLeave={onLeave}
            t={t}
          />
        )
      }
      state={state}
      status={status}
      actions={actions}
    />
  );
}

function pageName(t: Translate, title: string): string {
  return title.trim() === "" ? t("chat.proposal.page.untitled") : title;
}

interface BodyProps {
  workspaceId: string;
  view: PageProposalView;
  markdown: string;
  editing: boolean;
  tooLong: boolean;
  onEdit: (markdown: string) => void;
  onAdd: () => void;
  onStopEditing: () => void;
  onLeave: (() => void) | undefined;
  t: Translate;
}

/** Everything the member is confirming: who reads it, Notion, the text, the sources. */
function PageChangeBody({
  workspaceId,
  view,
  markdown,
  editing,
  tooLong,
  onEdit,
  onAdd,
  onStopEditing,
  onLeave,
  t,
}: BodyProps) {
  return (
    <>
      <div className="flex flex-col gap-1">
        <Text size="sm" tone="tertiary">
          {t("chat.proposal.page.readersLabel")}
        </Text>
        <Text>{readersSentence(t, view)}</Text>
        {view.notion ? (
          <Text size="sm" tone="secondary">
            {t("chat.proposal.page.notion")}
          </Text>
        ) : null}
      </div>
      {editing ? (
        <NoteField
          label={t("chat.proposal.page.editLabel")}
          value={markdown}
          onChange={onEdit}
          maxLength={MARKDOWN_MAX_BYTES}
          counter={(left) => t("chat.proposal.page.left", { count: left })}
          onSubmit={onAdd}
          onCancel={onStopEditing}
          error={tooLong ? t("chat.proposal.page.tooLong") : undefined}
        />
      ) : (
        <Clamp
          moreLabel={t("chat.proposal.page.showAll")}
          lessLabel={t("chat.proposal.page.showLess")}
        >
          <RichDocument>
            <PageMarkdown
              markdown={markdown}
              todoLabels={{
                done: t("chat.proposal.page.todoDone"),
                open: t("chat.proposal.page.todoOpen"),
              }}
            />
          </RichDocument>
        </Clamp>
      )}
      <Sources workspaceId={workspaceId} sources={view.sources} onLeave={onLeave} t={t} />
    </>
  );
}

const PLAIN = {
  "conversations.conversation": "chat.proposal.page.source.thread",
  "tasks.task": "chat.proposal.page.source.task",
  "schedule.event": "chat.proposal.page.source.event",
  "pages.page": "chat.proposal.page.source.page",
} as const;

/** The records the section draws on, as chips that open them. */
function Sources({
  workspaceId,
  sources,
  onLeave,
  t,
}: {
  workspaceId: string;
  sources: readonly RecordRef[];
  onLeave: (() => void) | undefined;
  t: Translate;
}) {
  const navigate = useNavigate();
  const threads = sources.filter((source) => source.type === "conversations.conversation");
  const found = useConversationsByIds(
    workspaceId,
    threads.map((source) => source.id),
  );
  if (sources.length === 0) {
    return null;
  }
  return (
    <ChipRow label={t("chat.proposal.page.sources")}>
      {sources.map((source) => {
        const summary = found.data?.find((candidate) => candidate.id === source.id);
        const plain = Object.hasOwn(PLAIN, source.type)
          ? t(PLAIN[source.type as keyof typeof PLAIN])
          : t("chat.proposal.page.source.other");
        const label =
          summary === undefined
            ? plain
            : summary.subject === ""
              ? t("chat.noSubject")
              : summary.subject;
        const target =
          source.type === "conversations.conversation"
            ? summary === undefined
              ? undefined
              : `/w/${workspaceId}/inbox/${summary.stream}/${summary.id}`
            : source.type === "tasks.task"
              ? `/w/${workspaceId}/tasks/${source.id}`
              : source.type === "pages.page"
                ? `/w/${workspaceId}/pages/${source.id}`
                : undefined;
        return target === undefined ? (
          <Chip key={`${source.type}:${source.id}`} disabled>
            {label}
          </Chip>
        ) : (
          <ChipLink
            key={`${source.type}:${source.id}`}
            href={target}
            onClick={(event) => {
              if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) {
                return;
              }
              event.preventDefault();
              onLeave?.();
              void navigate({ to: target });
            }}
          >
            {label}
          </ChipLink>
        );
      })}
    </ChipRow>
  );
}
