import type { Meta, StoryObj } from "@storybook/react-vite";
import { useTranslation } from "react-i18next";

import {
  Attachment,
  Banner,
  Button,
  ConversationRow,
  Dot,
  EmptyState,
  Line,
  NavItem,
  Note,
  SkeletonList,
  StatusLine,
  Text,
  Toast,
} from "~/lib/ui";

import { Board, BoardCell, BoardGrid } from "../board/Board";

const noop = () => {
  // Demo only: nothing to do.
};

function StatesBoard() {
  const { i18n } = useTranslation();
  // The Portuguese cell is rendered with the real pt-PT catalogue.
  const pt = i18n.getFixedT("pt-PT", "shortcuts");
  const ptUi = i18n.getFixedT("pt-PT", "ui");
  return (
    <Board title="States and edge cases">
      <BoardGrid>
        <BoardCell title="Loading">
          <SkeletonList rows={3} />
        </BoardCell>

        <BoardCell title="All clear">
          <EmptyState title="Nothing needs you" headingLevel={3}>
            Campbooks keeps reading and will bring things here when they do.
          </EmptyState>
        </BoardCell>

        <BoardCell title="Empty stream">
          <EmptyState
            title="Nothing in Clients"
            visor="idle"
            visorSize="lg"
            headingLevel={3}
            action={<Button>Edit the rule</Button>}
          >
            Mail that matches this stream’s rule will land here.
          </EmptyState>
        </BoardCell>

        <BoardCell title="Offline">
          <Banner title="You are offline.">2 changes will sync when you reconnect.</Banner>
          <Text tone="tertiary" size="sm">
            Reading, search and drafting keep working from the local copy.
          </Text>
        </BoardCell>

        <BoardCell title="Mailbox needs you">
          <Banner
            tone="danger"
            title="ana@duartepessoa.example stopped syncing."
            action={<Button>Reconnect</Button>}
          >
            Google asked you to sign in again.
          </Banner>
        </BoardCell>

        <BoardCell title="Send failed">
          <Toast tone="alert">
            <b>Not sent to Sofia.</b> The attachment is over the 25 MB limit of her mail server.
            Your draft is safe.
          </Toast>
          <div className="flex flex-wrap gap-2">
            <Button variant="primary" shortcut="list.open">
              Send as a link
            </Button>
            <Button variant="ghost">Edit draft</Button>
          </div>
        </BoardCell>

        <BoardCell title="Conversation failed to load">
          <EmptyState
            title="Could not open this conversation"
            visor="thinking"
            visorSize="lg"
            headingLevel={3}
            action={<Button keys="r">Try again</Button>}
          >
            Google did not answer in time. Nothing was lost.
          </EmptyState>
        </BoardCell>

        <BoardCell title="Low confidence">
          <Note
            visor="unsure"
            choice={{
              prompt: "Where does this go?",
              options: [
                { label: "Primary", keys: "1", onSelect: noop },
                { label: "Updates", keys: "2", onSelect: noop },
              ],
            }}
            confidence="61% sure. Below 80% Campbooks asks instead of acting."
          >
            An invitation to a workshop. Campbooks has not seen this sender before.
          </Note>
        </BoardCell>

        <BoardCell title="AI paused">
          <StatusLine visor="asleep" title="Campbooks is paused">
            Mail still arrives in the Inbox, unsorted. Nothing is read by AI.
          </StatusLine>
          <div>
            <Button>Resume</Button>
          </div>
        </BoardCell>

        <BoardCell title="Long names and subjects">
          <ConversationRow
            unread
            from="Maria do Carmo Sant’Ana Albuquerque e Vasconcelos"
            time="09:41"
            subject="Re: Re: Fwd: Proposta de alteração ao contrato de prestação de serviços de contabilidade 2027"
            snippet="Segue em anexo a versão revista com as alterações acordadas na reunião"
            onSelect={noop}
          />
          <Attachment name="contrato-prestacao-servicos-contabilidade-2027-versao-revista-final-assinada.pdf" />
        </BoardCell>

        <BoardCell title="Português (pt-PT)" lang="pt-PT">
          <div className="flex flex-col gap-0-5">
            <NavItem label={pt("nav.home")} count={6} current />
            <NavItem label={pt("nav.inbox")} count={7} shortcut="nav.inbox" />
            <NavItem label={pt("nav.schedule")} />
            <NavItem label={pt("nav.documents")} />
          </div>
          <Text as="div" tone="secondary" size="sm" weight="semibold">
            <span className="flex items-center gap-2">
              <Dot signal="decide" />
              {ptUi("signal.decide")}
            </span>
          </Text>
          <div className="flex flex-wrap gap-2">
            <Button variant="primary" keys="1">
              De 2 a 6 de nov.
            </Button>
            <Button keys="2">De 16 a 20 de nov., coincide com o Porto</Button>
            <Button variant="ghost" shortcut="item.later">
              {pt("item.later")}
            </Button>
          </div>
        </BoardCell>

        <BoardCell title="Undo, for every action">
          <Line trailing={<Button variant="link">Undo</Button>}>Done · Sofia Pessoa</Line>
          <Line trailing={<Button variant="link">Undo</Button>}>
            Later, back tomorrow · Rui Matos
          </Line>
          <Line undone trailing={<Button variant="link">Redo</Button>}>
            Mark as paid · Cloudhost
          </Line>
          <Text tone="tertiary" size="sm">
            Z undoes the most recent; the list keeps the rest for the session.
          </Text>
        </BoardCell>
      </BoardGrid>
    </Board>
  );
}

const meta = {
  title: "Patterns/States",
  parameters: { layout: "fullscreen" },
  render: () => <StatesBoard />,
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** Loading, empty, offline, failures, low confidence, paused, long content, Portuguese length, undo. */
export const Light: Story = {};

export const Dark: Story = { globals: { theme: "dark" } };
