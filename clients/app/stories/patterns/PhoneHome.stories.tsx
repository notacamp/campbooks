import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { expect, userEvent, waitFor, within } from "storybook/test";

import {
  IconButton,
  Item,
  PageHero,
  QueueGroup,
  Spacer,
  SwipeAction,
  TabBar,
  UndoProvider,
  useUndoToast,
  Visor,
  WorkspaceSwitcher,
} from "~/lib/ui";

import { SIGNAL_ORDER } from "./demo-data";
import styles from "./Phone.module.css";

const noop = () => {
  // Demo only: nothing to do.
};

const ITEMS = [
  {
    id: "m5",
    signal: "review",
    who: "Bankly",
    service: true,
    due: "Today",
    soon: true,
    what: "New sign-in from Lisbon",
  },
  {
    id: "m1",
    signal: "reply",
    who: "Sofia Pessoa",
    due: "Thu",
    what: "Waiting on your review of the Q4 proposal",
    swiped: true,
  },
  {
    id: "m2",
    signal: "decide",
    who: "Marta Duarte",
    due: "Wed",
    what: "Pick a week for the office move",
  },
  { id: "m3", signal: "do", who: "Rui Matos", due: "Fri", what: "Send two August receipts" },
  {
    id: "m4",
    signal: "pay",
    who: "Cloudhost",
    service: true,
    due: "Fri",
    what: "Invoice CH-20418, €148.00",
  },
] as const;

function PhoneHome() {
  const { t } = useTranslation("shortcuts");
  const { t: tUi } = useTranslation("ui");
  const undo = useUndoToast();
  const [gone, setGone] = useState<string[]>([]);
  const live = ITEMS.filter((item) => !gone.includes(item.id));
  const act = (item: (typeof ITEMS)[number], label: string) => {
    setGone((ids) => [...ids, item.id]);
    undo.push({
      label: `${label} · ${item.who}`,
      undo: () => {
        setGone((ids) => ids.filter((id) => id !== item.id));
      },
    });
  };
  return (
    <div className={styles["phone"]}>
      <header className={styles["header"]}>
        <WorkspaceSwitcher
          compact
          mark="DP"
          name="Duarte & Pessoa"
          kind="Business"
          onClick={noop}
        />
        <Spacer />
        <IconButton size="lg" label={t("command.open")} icon={<Visor size="md" />} />
      </header>
      <main className={styles["content"]}>
        <PageHero
          compact
          title={live.length === 0 ? "All clear" : `${String(live.length)} things need you`}
          subline="48 sorted since Friday"
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
                  avatar={{ name: item.who, kind: "service" in item ? "service" : "person" }}
                  due={item.due}
                  soon={"soon" in item}
                  hints={[]}
                  href="#conversation"
                  selected={"swiped" in item}
                  swipeActions={
                    "swiped" in item ? (
                      <>
                        <SwipeAction
                          tone="later"
                          icon="clock"
                          label={t("item.later")}
                          onClick={() => {
                            act(item, t("item.later"));
                          }}
                        />
                        <SwipeAction
                          tone="done"
                          icon="check"
                          label={t("item.done")}
                          onClick={() => {
                            act(item, t("item.done"));
                          }}
                        />
                      </>
                    ) : undefined
                  }
                />
              ))}
            </QueueGroup>
          );
        })}
      </main>
      <div className={styles["tabs"]}>
        <TabBar
          items={[
            { id: "home", label: t("nav.home"), icon: "home", current: true, badge: live.length },
            { id: "inbox", label: t("nav.inbox"), icon: "inbox" },
            { id: "schedule", label: t("nav.schedule"), icon: "schedule" },
            { id: "documents", label: t("nav.documents"), icon: "document" },
            { id: "money", label: t("nav.money"), icon: "money" },
          ]}
        />
      </div>
    </div>
  );
}

const meta = {
  title: "Patterns/Phone home",
  parameters: { layout: "fullscreen" },
  globals: { theme: "dark" },
  render: () => (
    <UndoProvider>
      <PhoneHome />
    </UndoProvider>
  ),
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** The queue on a phone: 44px rows, a swiped row revealing Later and Done, the tab bar. */
export const Dark: Story = {};

export const Light: Story = { globals: { theme: "light" } };

export const Personal: Story = { globals: { accent: "personal" } };

/** Swipe action, then Undo from the toast. */
export const DoneThenUndo: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.click(canvas.getByRole("button", { name: "Done" }));
    await expect(
      canvas.getByRole("heading", { level: 1, name: "4 things need you" }),
    ).toBeVisible();
    const body = within(canvasElement.ownerDocument.body);
    await userEvent.click(body.getByRole("button", { name: "Undo" }));
    await waitFor(() =>
      expect(canvas.getByRole("heading", { level: 1, name: "5 things need you" })).toBeVisible(),
    );
  },
};
