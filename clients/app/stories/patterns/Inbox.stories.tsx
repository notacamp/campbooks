import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  Composer,
  ConversationRow,
  EmptyState,
  Heading,
  HowDecided,
  IconButton,
  ListPane,
  MessageCard,
  MetaLine,
  NavItem,
  Note,
  PaneHeader,
  ReaderBody,
  ReaderPane,
  SearchButton,
  Segmented,
  Shell,
  Sidebar,
  SidebarFooter,
  Spacer,
  StatusLine,
  UndoProvider,
  useUndoToast,
  WorkspaceSwitcher,
} from "~/lib/ui";

import { STREAMS } from "./demo-data";

const noop = () => {
  // Demo only: nothing to do.
};

function InboxScreen({
  disconnected,
  initialStream,
}: {
  disconnected: boolean;
  initialStream: string;
}) {
  const { t } = useTranslation("shortcuts");
  const undo = useUndoToast();
  const [streamId, setStreamId] = useState(initialStream);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const [show, setShow] = useState<"all" | "unread">("all");
  const [done, setDone] = useState<string[]>([]);
  const [draft, setDraft] = useState("");

  const stream = STREAMS.find((entry) => entry.id === streamId) ?? STREAMS[0];
  const conversations = (stream?.conversations ?? [])
    .filter((conversation) => !done.includes(conversation.id))
    .filter((conversation) => show === "all" || conversation.unread);
  const current = conversations.find((entry) => entry.id === conversationId) ?? conversations[0];

  const move = (step: number) => {
    const index = conversations.findIndex((entry) => entry.id === current?.id);
    const next = conversations[Math.min(conversations.length - 1, Math.max(0, index + step))];
    if (next !== undefined) {
      setConversationId(next.id);
    }
  };
  const markDone = () => {
    if (current === undefined) {
      return;
    }
    setDone((ids) => [...ids, current.id]);
    undo.push({
      label: `${t("item.done")} · ${current.subject}`,
      undo: () => {
        setDone((ids) => ids.filter((id) => id !== current.id));
        setConversationId(current.id);
      },
    });
  };
  useShortcut("list.next", () => {
    move(1);
  });
  useShortcut("list.previous", () => {
    move(-1);
  });
  useShortcut("item.done", markDone);

  const first = current?.from.split(" ", 1)[0] ?? "";

  return (
    <Shell>
      <Sidebar>
        <WorkspaceSwitcher mark="DP" name="Duarte & Pessoa" kind="Business" onClick={noop} />
        <SearchButton onClick={noop} />
        <NavItem label={t("nav.home")} icon="home" count={6} shortcut="nav.home" href="#home" />
        <NavItem label={t("nav.inbox")} icon="inbox" count={7} shortcut="nav.inbox" current />
        {STREAMS.map((entry) => (
          <NavItem
            key={entry.id}
            label={entry.name}
            count={entry.count}
            sub
            current={entry.id === streamId}
            onClick={() => {
              setStreamId(entry.id);
              setConversationId(null);
            }}
          />
        ))}
        <NavItem label={t("nav.schedule")} icon="schedule" shortcut="nav.schedule" />
        <NavItem label={t("nav.tasks")} icon="check" count={7} shortcut="nav.tasks" />
        <NavItem label={t("nav.documents")} icon="document" shortcut="nav.documents" />
        <NavItem label={t("nav.money")} icon="money" shortcut="nav.money" />
        <SidebarFooter>
          <StatusLine visor="reading" title="Reading 3 new messages">
            48 sorted since Friday
          </StatusLine>
        </SidebarFooter>
      </Sidebar>

      <ListPane label="Conversations">
        <PaneHeader>
          <Heading level={1} size="pane">
            {stream?.name}
          </Heading>
          <Spacer />
          <Segmented
            label="Show"
            value={show}
            onChange={setShow}
            options={[
              { value: "all", label: "All" },
              { value: "unread", label: "Unread" },
            ]}
          />
        </PaneHeader>
        <div className="flex flex-col gap-0-5 p-1-5">
          {disconnected ? (
            <Banner
              tone="danger"
              title="ana@duartepessoa.example stopped syncing."
              action={<Button>Reconnect</Button>}
            >
              Google asked you to sign in again.
            </Banner>
          ) : null}
          {conversations.map((conversation) => (
            <ConversationRow
              key={conversation.id}
              from={conversation.from}
              avatar={{ kind: conversation.service === true ? "service" : "person" }}
              time={conversation.time}
              subject={conversation.subject}
              snippet={conversation.snippet}
              unread={conversation.unread}
              current={conversation.id === current?.id}
              onSelect={() => {
                setConversationId(conversation.id);
              }}
            />
          ))}
        </div>
        {conversations.length === 0 ? (
          <EmptyState
            title={`Nothing in ${stream?.name ?? ""}`}
            visor="happy"
            visorSize="lg"
            action={<Button>Edit the rule</Button>}
          >
            Mail that matches this stream’s rule will land here.
          </EmptyState>
        ) : null}
      </ListPane>

      <ReaderPane label="Conversation">
        <PaneHeader>
          <Button shortcut="item.done" onClick={markDone}>
            {t("item.done")}
          </Button>
          <Button variant="ghost" shortcut="item.later">
            {t("item.later")}
          </Button>
          <Button variant="ghost" shortcut="item.reply">
            {t("item.reply")}
          </Button>
          <Spacer />
          <IconButton
            label={t("list.previous")}
            icon="chevron-up"
            shortcut="list.previous"
            onClick={() => {
              move(-1);
            }}
          />
          <IconButton
            label={t("list.next")}
            icon="chevron-down"
            shortcut="list.next"
            onClick={() => {
              move(1);
            }}
          />
        </PaneHeader>
        {current === undefined ? null : (
          <ReaderBody>
            <div className="flex flex-col gap-1">
              <Heading level={2} size="page">
                {current.subject}
              </Heading>
              <MetaLine
                parts={[
                  current.from,
                  current.org,
                  current.messages.length === 1
                    ? "1 message"
                    : `${String(current.messages.length)} messages`,
                ]}
              />
            </div>
            <Note
              key={current.id}
              visor={current.unsure === true ? "unsure" : "idle"}
              links={current.links.map((link) => ({ ...link, onSelect: noop }))}
              how={
                <HowDecided
                  rows={current.why.map((answer) => ({
                    question: "Stream",
                    answer,
                    source: current.sortedLine,
                  }))}
                />
              }
              footer={<Button variant="link">Move</Button>}
              {...(current.unsure === true
                ? {
                    choice: {
                      prompt: "Where does this go?",
                      options: [
                        { label: "Primary", keys: "1", onSelect: noop },
                        { label: "Updates", keys: "2", onSelect: noop },
                      ],
                    },
                  }
                : {})}
            >
              {current.note}
            </Note>
            {current.messages.map((message) => (
              <MessageCard
                key={message.time}
                from={message.from}
                avatar={{ kind: current.service === true ? "service" : "person" }}
                time={message.time}
                attachment={message.file}
              >
                {message.body}
              </MessageCard>
            ))}
            <Composer
              label={`Reply to ${first}`}
              value={draft}
              onChange={setDraft}
              onSend={() => {
                const sent = draft;
                setDraft("");
                undo.push({
                  label: "Sending in 5 seconds",
                  undo: () => {
                    setDraft(sent);
                  },
                });
              }}
              onDraft={() => {
                setDraft(`Hi ${first}, thanks. Thursday works for me.`);
              }}
            />
          </ReaderBody>
        )}
      </ReaderPane>
    </Shell>
  );
}

const meta = {
  title: "Patterns/Inbox",
  parameters: { layout: "fullscreen" },
  args: { disconnected: false, initialStream: "primary" },
  render: (args) => (
    <UndoProvider>
      <InboxScreen {...args} />
    </UndoProvider>
  ),
} satisfies Meta<{ disconnected: boolean; initialStream: string }>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Streams (Primary is the human stream), the list, and the reading view with the assistant's note. */
export const Primary: Story = {};

/** The low-confidence case: the assistant asks instead of acting. */
export const LowConfidence: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: /Ana Lopes/ }));
    await expect(canvas.getByText("Where does this go?")).toBeVisible();
  },
};

export const MailboxDisconnected: Story = { args: { disconnected: true } };

export const EmptyStream: Story = { args: { initialStream: "clients" } };

export const Dark: Story = { globals: { theme: "dark" } };

/** J/K walk the list; "Why?" explains the sorting. */
export const KeyboardFlow: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.keyboard("j");
    await waitFor(() =>
      expect(
        canvas.getByRole("heading", { level: 2, name: "Workshop on 20 October, a few seats left" }),
      ).toBeVisible(),
    );
    await userEvent.keyboard("k");
    await userEvent.click(canvas.getByRole("button", { name: "Why?" }));
    await expect(canvas.getByText("You have replied to Sofia 14 times this year.")).toBeVisible();
  },
};
