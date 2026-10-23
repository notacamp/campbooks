import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import {
  Agenda,
  AgendaDay,
  AgendaRow,
  CalendarHeader,
  DayStrip,
  FoundItem,
  FoundReading,
  FoundStrip,
  KIND_ICONS,
  MonthGrid,
  SourceLink,
  ViewTabs,
  WeekGrid,
  type WeekBlock,
} from "./Calendar";
import { CALENDAR_KINDS } from "../../tokens/tokens";

describe("kinds", () => {
  it("has an icon for every kind", () => {
    expect(Object.keys(KIND_ICONS).toSorted()).toEqual([...CALENDAR_KINDS].toSorted());
  });
});

describe("AgendaRow", () => {
  const row = (props: Partial<Parameters<typeof AgendaRow>[0]> = {}) => (
    <Agenda>
      <AgendaDay id="agenda-2026-10-27" name="Tuesday" sub="27 October" today="Today">
        <AgendaRow
          kind="travel"
          when="07:40 – 11:25"
          title="Flight LIS → AMS"
          kindLabel="Travel"
          meta="Lisbon T1"
          from={{ label: "From email", sender: "Aerolis" }}
          selected={false}
          onSelect={vi.fn()}
          {...props}
        />
      </AgendaDay>
    </Agenda>
  );

  it("says what, when and where, and that Scout read it from an email", () => {
    renderUi(row());

    const button = screen.getByRole("button", { name: /Flight LIS → AMS/u });
    expect(button).toHaveAttribute("data-kind", "travel");
    expect(within(button).getAllByText("07:40 – 11:25").length).toBeGreaterThan(0);
    expect(within(button).getByText("From email")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /Tuesday/u })).toBeVisible();
    expect(screen.getByText("Today")).toBeVisible();
  });

  it("marks the open event, and presses to select it", () => {
    const onSelect = vi.fn();
    renderUi(row({ selected: true, onSelect }));

    const button = screen.getByRole("button", { name: /Flight LIS → AMS/u });
    fireEvent.click(button);

    expect(button).toHaveAttribute("aria-current", "true");
    expect(onSelect).toHaveBeenCalledOnce();
  });

  it("marks an overdue time", () => {
    renderUi(row({ when: "Overdue · 2 days", overdue: true, kind: "due" }));

    expect(screen.getAllByText("Overdue · 2 days")[0]).toHaveAttribute("data-tone", "overdue");
  });
});

describe("ViewTabs", () => {
  it("presses the view shown and picks another", () => {
    const onChange = vi.fn();
    renderUi(
      <ViewTabs
        label="View"
        value="agenda"
        options={[
          { value: "agenda", label: "Agenda", shortcut: "nav.home" },
          { value: "week", label: "Week", shortcut: "nav.inbox" },
        ]}
        onChange={onChange}
      />,
    );

    expect(screen.getByRole("button", { name: /Agenda/u })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: /Week/u }));

    expect(onChange).toHaveBeenCalledWith("week");
  });
});

describe("CalendarHeader", () => {
  it("has the title, the period, the views and the toolbar", () => {
    renderUi(
      <CalendarHeader
        title="Calendar"
        period="Tuesday – Monday · Lisbon time"
        views={<span>views</span>}
        actions={<span>actions</span>}
      />,
    );

    expect(screen.getByRole("heading", { level: 1, name: "Calendar" })).toBeVisible();
    expect(screen.getByText("Tuesday – Monday · Lisbon time")).toBeVisible();
    expect(screen.getByText("views")).toBeVisible();
    expect(screen.getByText("actions")).toBeVisible();
  });
});

describe("DayStrip", () => {
  it("shows a dot per thing, at most three, and goes to the day", () => {
    const onSelect = vi.fn();
    renderUi(
      <DayStrip
        label="This week"
        days={[
          {
            key: "a",
            weekday: "Mon",
            num: "26",
            label: "Monday 26",
            today: false,
            dots: 1,
            onSelect,
          },
          {
            key: "b",
            weekday: "Tue",
            num: "27",
            label: "Tuesday 27",
            today: true,
            dots: 5,
            onSelect,
          },
        ]}
      />,
    );

    const today = screen.getByRole("button", { name: "Tuesday 27" });
    expect(today).toHaveAttribute("aria-current", "date");
    expect(today.querySelectorAll("span[aria-hidden] > span")).toHaveLength(3);
    fireEvent.click(screen.getByRole("button", { name: "Monday 26" }));
    expect(onSelect).toHaveBeenCalledOnce();
  });
});

describe("WeekGrid", () => {
  const block = (fields: Partial<WeekBlock>): WeekBlock => ({
    id: "a",
    kind: "meeting",
    title: "Sync",
    when: "09:30 – 10:00",
    start: 150,
    length: 30,
    column: 0,
    columns: 1,
    selected: false,
    onSelect: vi.fn(),
    ...fields,
  });
  const days = ["Mon", "Tue"].map((weekday, index) => ({
    key: `d${String(index)}`,
    weekday,
    num: String(26 + index),
    label: `${weekday} ${String(26 + index)}`,
    today: index === 1,
  }));

  it("places an event by the minute and side by side with what it overlaps", () => {
    const first = block({ id: "a", title: "First", column: 0, columns: 2 });
    const second = block({ id: "b", title: "Second", column: 1, columns: 2 });
    renderUi(
      <WeekGrid
        label="Week"
        days={days}
        allDay={{ label: "All day", pills: [] }}
        hours={{ from: 7, to: 10, format: (hour) => `${String(hour)}:00` }}
        blocks={[[first, second], []]}
        now={{ day: 1, minute: 90 }}
      />,
    );

    const style = screen.getByRole("button", { name: /First/u }).getAttribute("style") ?? "";
    expect(style).toContain("--start: 150");
    expect(style).toContain("--len: 30");
    expect(style).toContain("--columns: 2");
    expect(screen.getByRole("button", { name: /Second/u }).getAttribute("style")).toContain(
      "--column: 1",
    );
    expect(screen.getByRole("region", { name: "Week" })).toBeVisible();
    expect(screen.getByText("8:00")).toBeInTheDocument();
  });

  it("has no Due lane without tasks", () => {
    renderUi(
      <WeekGrid
        label="Week"
        days={days}
        allDay={{ label: "All day", pills: [] }}
        hours={{ from: 7, to: 9, format: (hour) => String(hour) }}
        blocks={[[], []]}
      />,
    );

    expect(screen.queryByText("Due")).toBeNull();
    expect(screen.getByText("All day")).toBeVisible();
  });

  it("draws the Due lane, each task over the columns it covers", () => {
    const pill = {
      id: "p",
      kind: "due",
      title: "Pay Costa Obras",
      from: 2,
      to: 3,
      selected: false,
      dashed: true,
      onSelect: vi.fn(),
    } as const;
    renderUi(
      <WeekGrid
        label="Week"
        days={days}
        due={{ label: "Due", pills: [pill] }}
        allDay={{ label: "All day", pills: [] }}
        hours={{ from: 7, to: 9, format: (hour) => String(hour) }}
        blocks={[[], []]}
      />,
    );

    expect(screen.getByText("Due")).toBeVisible();
    expect(
      screen.getByRole("button", { name: /Pay Costa Obras/u }).getAttribute("style"),
    ).toContain("2 / 3");
  });
});

describe("MonthGrid", () => {
  it("shows a day's first events, '+N more' and opens the day", () => {
    const onOpen = vi.fn();
    const onSelect = vi.fn();
    renderUi(
      <MonthGrid
        label="Month"
        weekdays={["Mon"]}
        weeks={[
          [
            {
              key: "2026-10-26",
              num: "26",
              label: "Monday 26 October",
              today: false,
              outside: false,
              events: [{ id: "a", kind: "meeting", title: "Standup", selected: false, onSelect }],
              more: "+2 more",
              onOpen,
            },
          ],
        ]}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Standup" }));
    fireEvent.click(screen.getByRole("button", { name: "+2 more" }));
    fireEvent.click(screen.getByRole("button", { name: "Monday 26 October" }));

    expect(onSelect).toHaveBeenCalledOnce();
    expect(onOpen).toHaveBeenCalledTimes(2);
  });
});

describe("the strip in the reader", () => {
  it("names what was found, where it is now and what can be done", () => {
    const undo = vi.fn();
    renderUi(
      <FoundStrip label="Scout noticed">
        <FoundItem
          kind="appointment"
          title="Key handover at the flat"
          meta="Appointment · Saturday 31 October, 10:00"
          quote="The key handover is on Saturday 31 October at 10:00."
          status="On your Campbooks calendar"
          actions={<button onClick={undo}>Undo</button>}
        />
      </FoundStrip>,
    );

    const strip = screen.getByRole("region", { name: "Scout noticed" });
    expect(within(strip).getByText("Key handover at the flat")).toBeVisible();
    expect(within(strip).getByText("On your Campbooks calendar")).toBeVisible();
    fireEvent.click(within(strip).getByRole("button", { name: "Undo" }));
    expect(undo).toHaveBeenCalledOnce();
  });

  it("says Scout is reading", () => {
    renderUi(
      <FoundStrip label="Scout noticed">
        <FoundReading>Scout is reading this…</FoundReading>
      </FoundStrip>,
    );

    expect(screen.getByRole("status")).toHaveTextContent("Scout is reading this…");
  });
});

describe("SourceLink", () => {
  it("links to the message an event was read from", () => {
    renderUi(
      <SourceLink
        href="/w/1/inbox/updates/2"
        label="From email"
        subject="Booking confirmed"
        meta="Aerolis · 21 October"
      />,
    );

    const link = screen.getByRole("link", { name: /Booking confirmed/u });
    expect(link).toHaveAttribute("href", "/w/1/inbox/updates/2");
    expect(link).toHaveTextContent("From email");
  });
});
