import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { expect, userEvent, waitFor, within } from "storybook/test";

import { useShortcut } from "~/lib/shortcuts";
import {
  AsideSection,
  Avatar,
  Button,
  CommandMenu,
  Dot,
  EmptyState,
  Figure,
  FigureRow,
  HomeLayout,
  Item,
  ItemDetail,
  Line,
  LineMeta,
  NavItem,
  PageHero,
  QueueGroup,
  SearchButton,
  Shell,
  ShortcutSheet,
  Sidebar,
  SidebarFooter,
  StatusLine,
  TabBar,
  UndoProvider,
  useUndoToast,
  Visor,
  WorkspaceSwitcher,
  type Command,
} from "~/lib/ui";

import { SIGNAL_ORDER, WORKSPACES, type QueueItemData } from "./demo-data";

const noop = () => {
  // Demo only: nothing to do.
};

function HomeScreen({ initialWorkspace }: { initialWorkspace: "business" | "personal" }) {
  const { t } = useTranslation("shortcuts");
  const { t: tUi } = useTranslation("ui");
  const undo = useUndoToast();
  const [workspace, setWorkspace] = useState(initialWorkspace);
  const ws = WORKSPACES[workspace];
  const [gone, setGone] = useState<string[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [open, setOpen] = useState<string | null>(initialWorkspace === "business" ? "b1" : null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [sheetOpen, setSheetOpen] = useState(false);
  const [undoneLog, setUndoneLog] = useState<string[]>([]);

  const live = ws.items.filter((item) => !gone.includes(item.id));
  const order = SIGNAL_ORDER.flatMap(({ signal }) => live.filter((item) => item.signal === signal));
  const current = order.find((item) => item.id === selected) ?? order[0];

  const act = (item: QueueItemData, label: string) => {
    setGone((ids) => [...ids, item.id]);
    setOpen(null);
    undo.push({
      label: `${label} · ${item.who}`,
      undo: () => {
        setGone((ids) => ids.filter((id) => id !== item.id));
        setSelected(item.id);
      },
    });
  };
  const move = (step: number) => {
    const index = order.findIndex((item) => item.id === current?.id);
    const next = order[Math.min(order.length - 1, Math.max(0, index + step))];
    if (next !== undefined) {
      setSelected(next.id);
    }
  };

  useShortcut("list.next", () => {
    move(1);
  });
  useShortcut("list.previous", () => {
    move(-1);
  });
  useShortcut("list.open", () => {
    if (current !== undefined) {
      setOpen((id) => (id === current.id ? null : current.id));
    }
  });
  useShortcut("item.done", () => {
    if (current !== undefined) {
      act(current, t("item.done"));
    }
  });
  useShortcut("item.snooze", () => {
    if (current !== undefined) {
      act(current, "Snoozed, back tomorrow");
    }
  });
  useShortcut("command.open", () => {
    setSheetOpen(false);
    setMenuOpen((value) => !value);
  });
  useShortcut("sheet.open", () => {
    setMenuOpen(false);
    setSheetOpen((value) => !value);
  });
  const switchWorkspace = () => {
    setWorkspace((key) => (key === "business" ? "personal" : "business"));
    setGone([]);
    setOpen(null);
    setSelected(null);
  };
  useShortcut("workspace.switch", switchWorkspace);
  useShortcut("nav.home", noop);
  useShortcut("nav.inbox", noop);

  const count = order.length;
  const hasAlert = live.some((item) => item.signal === "review");
  const suggestions: Command[] =
    current === undefined
      ? []
      : current.buttons.map((button) => ({
          id: `${current.id}-${button.label}`,
          label: `${button.label} · ${current.who}`,
          group: "Suggested",
          ...(button.shortcut === undefined ? {} : { shortcut: button.shortcut }),
          onSelect: () => {
            act(current, button.label);
          },
        }));

  return (
    <div data-accent={workspace}>
      <Shell>
        <Sidebar>
          <WorkspaceSwitcher
            mark={ws.mark}
            name={ws.name}
            kind={ws.kind}
            onClick={switchWorkspace}
          />
          <SearchButton
            onClick={() => {
              setMenuOpen(true);
            }}
          />
          <NavItem label={t("nav.home")} icon="home" count={count} shortcut="nav.home" current />
          <NavItem label={t("nav.inbox")} icon="inbox" count={ws.unread} shortcut="nav.inbox" />
          {ws.streams.map((stream) => (
            <NavItem key={stream.name} label={stream.name} count={stream.count} sub />
          ))}
          <NavItem label={t("nav.schedule")} icon="schedule" shortcut="nav.schedule" />
          <NavItem label={t("nav.tasks")} icon="check" count={ws.tasks} shortcut="nav.tasks" />
          <NavItem label={t("nav.documents")} icon="document" shortcut="nav.documents" />
          <NavItem label={t("nav.money")} icon="money" shortcut="nav.money" />
          <SidebarFooter>
            <StatusLine visor="reading" title="Reading 3 new messages">
              {ws.sorted} sorted since Friday
            </StatusLine>
            <Button
              variant="ghost"
              icon="keyboard"
              shortcut="sheet.open"
              onClick={() => {
                setSheetOpen(true);
              }}
            >
              {t("sheet.open")}
            </Button>
          </SidebarFooter>
        </Sidebar>

        <HomeLayout
          asideLabel="Context"
          aside={
            <>
              <AsideSection title="Today">
                {ws.today.map((event) => (
                  <Line
                    key={event.time}
                    time={event.time}
                    leading={<Dot signal={event.signal} />}
                    trailing={<LineMeta>{event.where}</LineMeta>}
                  >
                    {event.title}
                  </Line>
                ))}
              </AsideSection>
              <AsideSection title={t("nav.money")}>
                <FigureRow>
                  <Figure value={ws.money.due} label={ws.money.dueLabel} />
                  <Figure value={ws.money.incoming} label={ws.money.incomingLabel} />
                </FigureRow>
                <Line leading={<Dot signal="pay" />}>{ws.money.statement}</Line>
              </AsideSection>
              <AsideSection title="Waiting on others">
                {ws.waiting.map((entry) => (
                  <Line
                    key={entry.text}
                    leading={
                      <Avatar
                        name={entry.name}
                        kind={entry.service === true ? "service" : "person"}
                        size="sm"
                      />
                    }
                    trailing={<Button variant="ghost">Nudge</Button>}
                  >
                    {entry.text}
                  </Line>
                ))}
              </AsideSection>
              <AsideSection title="Sorted since Friday">
                {ws.streams.map((stream) => (
                  <Line
                    key={stream.name}
                    href="#inbox"
                    trailing={<LineMeta>{stream.fresh}</LineMeta>}
                  >
                    {stream.name}
                  </Line>
                ))}
              </AsideSection>
              <AsideSection title="What Campbooks did" leading={<Visor />}>
                {ws.log.map((text) => {
                  const isUndone = undoneLog.includes(text);
                  return (
                    <Line
                      key={text}
                      undone={isUndone}
                      trailing={
                        <Button
                          variant="link"
                          onClick={() => {
                            setUndoneLog((list) =>
                              isUndone ? list.filter((entry) => entry !== text) : [...list, text],
                            );
                          }}
                        >
                          {isUndone ? tUi("redo") : tUi("undo")}
                        </Button>
                      }
                    >
                      {text}
                    </Line>
                  );
                })}
              </AsideSection>
            </>
          }
        >
          <PageHero
            visor={count === 0 ? "happy" : hasAlert ? "alert" : "idle"}
            dateLine={ws.date}
            title={
              count === 0
                ? "All clear"
                : count === 1
                  ? "1 thing needs you"
                  : `${String(count)} things need you`
            }
            subline={`${String(ws.sorted)} messages sorted since Friday. Campbooks handled the rest.`}
          />
          {SIGNAL_ORDER.map(({ signal }) => {
            const items = live.filter((item) => item.signal === signal);
            if (items.length === 0) {
              return null;
            }
            return (
              <QueueGroup
                key={signal}
                signal={signal}
                name={tUi(`signal.${signal}`)}
                count={items.length}
              >
                {items.map((item) => (
                  <Item
                    key={item.id}
                    who={item.who}
                    what={item.what}
                    avatar={{ name: item.name, kind: item.service === true ? "service" : "person" }}
                    due={item.due}
                    soon={item.soon === true}
                    tags={item.tags}
                    selected={item.id === current?.id}
                    expanded={open === item.id}
                    onToggle={() => {
                      setSelected(item.id);
                      setOpen((id) => (id === item.id ? null : item.id));
                    }}
                    detail={
                      <ItemDetail
                        quoteFrom={item.quoteFrom}
                        quote={item.quote}
                        context={item.context}
                        actions={
                          <>
                            {item.buttons.map((button) => (
                              <Button
                                key={button.label}
                                variant={button.variant}
                                keys={button.keys}
                                shortcut={button.shortcut}
                                onClick={() => {
                                  act(item, button.label);
                                }}
                              >
                                {button.label}
                              </Button>
                            ))}
                            <Button
                              variant="ghost"
                              shortcut="item.snooze"
                              onClick={() => {
                                act(item, "Snoozed, back tomorrow");
                              }}
                            >
                              {t("item.snooze")}
                            </Button>
                            <Button
                              variant="ghost"
                              shortcut="item.done"
                              onClick={() => {
                                act(item, t("item.done"));
                              }}
                            >
                              {t("item.done")}
                            </Button>
                          </>
                        }
                      />
                    }
                  />
                ))}
              </QueueGroup>
            );
          })}
          {count === 0 ? (
            <EmptyState title="Nothing needs you">
              Campbooks keeps reading and will bring things here when they do.
            </EmptyState>
          ) : null}
        </HomeLayout>
      </Shell>
      <TabBar
        phoneOnly
        items={[
          { id: "home", label: t("nav.home"), icon: "home", current: true, badge: count },
          { id: "inbox", label: t("nav.inbox"), icon: "inbox" },
          { id: "schedule", label: t("nav.schedule"), icon: "schedule" },
          { id: "documents", label: t("nav.documents"), icon: "document" },
          { id: "money", label: t("nav.money"), icon: "money" },
        ]}
      />
      <CommandMenu
        open={menuOpen}
        onClose={() => {
          setMenuOpen(false);
        }}
        commands={suggestions}
        onAsk={noop}
      />
      <ShortcutSheet
        open={sheetOpen}
        onClose={() => {
          setSheetOpen(false);
        }}
      />
    </div>
  );
}

const meta = {
  title: "Patterns/Home",
  parameters: { layout: "fullscreen" },
  render: (_, { globals }) => (
    <UndoProvider>
      <HomeScreen initialWorkspace={globals["accent"] === "personal" ? "personal" : "business"} />
    </UndoProvider>
  ),
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * "Needs you" as home. J/K move, ↵ opens, E done, L later, Z undo, ? shows
 * every shortcut, ⌘K searches, ⌘⇧W switches workspace (and its accent).
 */
export const Business: Story = {};

export const Personal: Story = { globals: { accent: "personal" } };

export const Dark: Story = { globals: { theme: "dark" } };

/** J moves the cursor, E marks it done, Z brings it back. */
export const KeyboardFlow: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(
      canvas.getByRole("heading", { level: 1, name: "9 things need you" }),
    ).toBeVisible();
    await userEvent.keyboard("j");
    await userEvent.keyboard("e");
    await waitFor(() =>
      expect(canvas.getByRole("heading", { level: 1, name: "8 things need you" })).toBeVisible(),
    );
    await userEvent.keyboard("z");
    await waitFor(() =>
      expect(canvas.getByRole("heading", { level: 1, name: "9 things need you" })).toBeVisible(),
    );
  },
};
