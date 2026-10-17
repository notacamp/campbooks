import type { Meta, StoryObj } from "@storybook/react-vite";
import { useTranslation } from "react-i18next";

import {
  ActionBar,
  Button,
  ButtonLink,
  Heading,
  IconButton,
  MessageCard,
  MetaLine,
  Note,
  Spacer,
  UndoProvider,
  useUndoToast,
  Visor,
} from "~/lib/ui";

import styles from "./Phone.module.css";

const noop = () => {
  // Demo only: nothing to do.
};

function PhoneConversation() {
  const { t } = useTranslation("shortcuts");
  const undo = useUndoToast();
  return (
    <div className={styles["phone"]}>
      <header className={styles["header"]}>
        <ButtonLink variant="ghost" size="lg" icon="chevron-left" href="#home">
          {t("nav.home")}
        </ButtonLink>
        <Spacer />
        <IconButton size="lg" label={t("item.snooze")} icon="clock" />
        <IconButton size="lg" label="More actions" icon="more" />
      </header>
      <main className={styles["thread"]}>
        <div className="flex flex-col gap-1">
          <Heading level={1} size="title">
            Q4 proposal: can you review it?
          </Heading>
          <MetaLine parts={["Sofia Pessoa", "Brightloop", "2 messages"]} />
        </div>
        <Note
          compact
          links={[
            { icon: "check", title: "Review Q4 proposal", onSelect: noop },
            { icon: "event", title: "Held Thu 09:00", onSelect: noop },
          ]}
        >
          Sofia is waiting on your review by Thursday. Version 3 moves the timeline two weeks; the
          price is unchanged.
        </Note>
        <MessageCard
          from="Sofia Pessoa"
          time="Fri 17:42"
          attachment="Brightloop-Q4-scope-v3-final-with-appendix.pdf"
        >
          Attached the revised scope. Thursday works for a call if that is easier; otherwise
          comments in the doc are great.
        </MessageCard>
        <MessageCard from="Sofia Pessoa" time="Tue 10:14">
          Hi Ana, here is the first draft of the Q4 proposal. Let me know what you think.
        </MessageCard>
      </main>
      <ActionBar label="Conversation actions">
        <Button variant="primary" size="lg" icon="reply" grow>
          {t("item.reply")}
        </Button>
        <Button size="lg" leading={<Visor />} aria-label={t("compose.draft")}>
          Draft
        </Button>
        <Button
          size="lg"
          onClick={() => {
            undo.push({ label: `${t("item.done")} · Sofia Pessoa`, undo: noop });
          }}
        >
          {t("item.done")}
        </Button>
      </ActionBar>
    </div>
  );
}

const meta = {
  title: "Patterns/Phone conversation",
  parameters: { layout: "fullscreen" },
  globals: { theme: "dark" },
  render: () => (
    <UndoProvider>
      <PhoneConversation />
    </UndoProvider>
  ),
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** The assistant's note first, then the messages; main actions in a floating bar. */
export const Dark: Story = {};

export const Light: Story = { globals: { theme: "light" } };
