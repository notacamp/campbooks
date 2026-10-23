import type { Meta, StoryObj } from "@storybook/react-vite";

import { CALENDAR_KINDS } from "../../tokens/tokens";

import {
  Agenda,
  AgendaDay,
  AgendaRow,
  FoundItem,
  FoundReading,
  FoundStrip,
  KindChip,
  KindIcon,
  MonthGrid,
  WeekGrid,
  type WeekBlock,
} from "./Calendar";

const noop = () => {
  // Demo only: nothing to do.
};

const meta = {
  title: "Components/Calendar",
  parameters: { layout: "padded" },
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** One colour and one icon per kind of dated thing; the colour carries the kind. */
export const Kinds: Story = {
  render: () => (
    <div className="flex flex-col gap-3">
      {CALENDAR_KINDS.map((kind) => (
        <div key={kind} className="flex items-center gap-3">
          <KindIcon kind={kind} />
          <KindChip kind={kind}>{kind}</KindChip>
        </div>
      ))}
    </div>
  ),
};

export const AgendaOfADay: Story = {
  render: () => (
    <Agenda>
      <AgendaDay id="agenda-demo" name="Tuesday" sub="27 October" today="Today">
        <AgendaRow
          kind="due"
          when="Overdue · 2 days"
          overdue
          title="Pay · EDP bill · €84.20"
          kindLabel="Due"
          meta="Task"
          from={{ label: "From email", sender: "EDP" }}
          selected={false}
          onSelect={noop}
        />
        <AgendaRow
          kind="meeting"
          when="09:30 – 10:00"
          title="Weekly sync with Rui Costa"
          kindLabel="Meeting"
          meta="Work"
          selected={false}
          onSelect={noop}
        />
        <AgendaRow
          kind="appointment"
          when="15:00 – 15:45"
          title="Dentist check-up"
          kindLabel="Appointment"
          meta="Clínica Sorriso, Rua Augusta 120"
          from={{ label: "From email", sender: "Clínica Sorriso" }}
          selected
          onSelect={noop}
        />
      </AgendaDay>
    </Agenda>
  ),
};

const block = (fields: Partial<WeekBlock>): WeekBlock => ({
  id: "a",
  kind: "meeting",
  title: "Weekly sync",
  when: "09:30 – 10:00",
  start: 150,
  length: 30,
  column: 0,
  columns: 1,
  selected: false,
  onSelect: noop,
  ...fields,
});

const WEEK_DAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map((weekday, index) => ({
  key: weekday,
  weekday,
  num: String(26 + index),
  label: weekday,
  today: index === 1,
}));

/** The week as a time grid: hatched held time, an event side by side with another, the now line. */
export const Week: Story = {
  parameters: { layout: "fullscreen" },
  render: () => (
    <WeekGrid
      label="Week"
      days={WEEK_DAYS}
      due={{
        label: "Due",
        pills: [
          {
            id: "p",
            kind: "due",
            title: "Pay Costa Obras · €1,240",
            from: 2,
            to: 3,
            dashed: true,
            selected: false,
            onSelect: noop,
          },
        ],
      }}
      allDay={{
        label: "All day",
        pills: [
          {
            id: "h",
            kind: "stay",
            title: "Hotel Canal Verde",
            from: 5,
            to: 8,
            selected: false,
            onSelect: noop,
          },
        ],
      }}
      hours={{ from: 8, to: 20, format: (hour) => `${String(hour).padStart(2, "0")}:00` }}
      blocks={[
        [],
        [
          block({ id: "a", column: 0, columns: 2 }),
          block({ id: "b", title: "Standup", kind: "meeting", column: 1, columns: 2 }),
          block({
            id: "c",
            title: "Held · Quote",
            kind: "hold",
            held: true,
            start: 180,
            length: 90,
          }),
          block({
            id: "d",
            title: "Dentist check-up",
            kind: "appointment",
            fromLabel: "From email",
            when: "15:00 – 15:45",
            start: 420,
            length: 45,
            selected: true,
          }),
        ],
        [],
        [],
        [],
        [],
        [],
      ]}
      now={{ day: 1, minute: 380 }}
    />
  ),
};

export const Month: Story = {
  render: () => (
    <MonthGrid
      label="Month"
      weekdays={["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]}
      weeks={[
        WEEK_DAYS.map((day, index) => ({
          key: day.key,
          num: day.num,
          label: day.label,
          today: day.today,
          outside: index === 6,
          events:
            index === 1
              ? [
                  {
                    id: "a",
                    kind: "meeting",
                    title: "Weekly sync",
                    selected: false,
                    onSelect: noop,
                  },
                  {
                    id: "b",
                    kind: "appointment",
                    title: "Dentist",
                    selected: false,
                    onSelect: noop,
                  },
                  { id: "c", kind: "due", title: "Pay EDP", selected: false, onSelect: noop },
                ]
              : [],
          more: index === 1 ? "+2 more" : undefined,
          onOpen: noop,
        })),
      ]}
    />
  ),
};

/** What Scout read from a message, and Scout reading one. */
export const InTheReader: Story = {
  render: () => (
    <div className="flex max-w-measure flex-col gap-4">
      <FoundStrip label="Scout noticed">
        <FoundItem
          kind="appointment"
          title="Key handover at the flat"
          meta="Appointment · Saturday 31 October, 10:00"
          quote="The key handover is on Saturday 31 October at 10:00 at the flat."
          status="On your Campbooks calendar"
        />
      </FoundStrip>
      <FoundStrip label="Scout noticed">
        <FoundReading>Scout is reading this…</FoundReading>
      </FoundStrip>
    </div>
  ),
};
