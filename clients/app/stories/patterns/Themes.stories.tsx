import type { Meta, StoryObj } from "@storybook/react-vite";

import {
  Button,
  Heading,
  Item,
  Pill,
  QueueGroup,
  Tag,
  Text,
  Visor,
  type Accent,
  type ThemeMode,
} from "~/lib/ui";

import { Board, BoardCell, BoardGrid, Code, ThemeTile } from "../board/Board";

const TILES: { label: string; theme: ThemeMode; accent: Accent }[] = [
  { label: "Business (ink) · light", theme: "light", accent: "business" },
  { label: "Business (ink) · dark", theme: "dark", accent: "business" },
  { label: "Personal (terracotta) · light", theme: "light", accent: "personal" },
  { label: "Personal (terracotta) · dark", theme: "dark", accent: "personal" },
  { label: "Forest · light", theme: "light", accent: "forest" },
  { label: "Forest · dark", theme: "dark", accent: "forest" },
];

function ThemesBoard() {
  return (
    <Board
      title="One token set, any theme"
      intro="The same markup below; only data-theme and data-accent change."
    >
      <BoardGrid>
        {TILES.map((tile) => (
          <ThemeTile key={tile.label} title={tile.label} theme={tile.theme} accent={tile.accent}>
            <div className="flex items-center gap-4">
              <Visor size="lg" />
              <div>
                <Heading level={3} size="title">
                  3 things need you
                </Heading>
                <Text tone="secondary" size="sm">
                  21 sorted since Friday
                </Text>
              </div>
            </div>
            <QueueGroup signal="reply" name="Reply" count={1}>
              <Item
                who="Sofia Pessoa"
                what="Waiting on your review"
                avatar={{ name: "Sofia Pessoa" }}
                due="Thu"
                selected
              />
              <Item
                who="Cloudhost"
                what="Invoice CH-20418, €148.00"
                avatar={{ name: "Cloudhost", kind: "service" }}
                due="Fri"
              />
            </QueueGroup>
            <div className="flex flex-wrap gap-2">
              <Button variant="primary" shortcut="list.open">
                Open
              </Button>
              <Button shortcut="item.hold">Hold time</Button>
              <Button variant="ghost" shortcut="item.done">
                Done
              </Button>
            </div>
            <div className="flex flex-wrap gap-2">
              <Pill signal="reply" />
              <Pill signal="decide" />
              <Pill signal="pay" />
              <Pill signal="review" />
              <Tag>Proposal</Tag>
            </div>
          </ThemeTile>
        ))}
      </BoardGrid>
      <BoardGrid>
        <BoardCell title="1 · Primitives">
          <Text size="sm">Raw hues and scales. A brand or workspace changes only these.</Text>
          <Code>
            {`--neutral-h, --neutral-c
--accent-h, --accent-c
--signal-reply-h … --signal-sort-h
--avatar-1-h … --avatar-8-h
--text-xs … --text-3xl, --weight-*
--space-0-5 … --space-12
--radius-xs … --radius-full
--duration-*, --ease-out`}
          </Code>
        </BoardCell>
        <BoardCell title="2 · Semantic">
          <Text size="sm">
            Roles, each defined once for light and dark with light-dark(). Components read these.
          </Text>
          <Code>
            {`--surface-base / raised / sunken / overlay
--surface-hover / selected / scrim
--text-primary / secondary / tertiary / disabled
--border-subtle / strong, --edge-highlight
--accent, --accent-hover, --on-accent, --accent-soft
--signal-reply … --signal-sort, --danger, --success
--visor-glass-*, --visor-eye, --shadow-overlay`}
          </Code>
        </BoardCell>
        <BoardCell title="3 · Component">
          <Text size="sm">Per-component knobs. Density and layout widths live here.</Text>
          <Code>
            {`--btn-height, --btn-radius, --btn-pad-x
--row-pad-y, --row-pad-x, --row-radius
--kbd-size, --kbd-bg, --kbd-fg, --kbd-border
--pill-height, --visor-size, --panel-radius
--sidebar-width, --list-width, --aside-width
data-density="compact" overrides a few`}
          </Code>
        </BoardCell>
      </BoardGrid>
    </Board>
  );
}

const meta = {
  title: "Patterns/Themes",
  parameters: { layout: "fullscreen" },
  render: () => <ThemesBoard />,
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** Business (ink), Personal (terracotta) and Forest, each in light and dark: one token set. */
export const AllThemes: Story = {};

export const Compact: Story = { globals: { density: "compact" } };
