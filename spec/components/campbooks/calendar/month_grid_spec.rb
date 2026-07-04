require "rails_helper"

RSpec.describe Campbooks::Calendar::MonthGrid, type: :component do
  def uuid_for(n)
    "00000000-0000-4000-8000-#{n.to_s.rjust(12, '0')}"
  end

  def calendar(color: "#2ea55c")
    Calendar.new(id: 1, name: "Personal", color: color, calendar_account: CalendarAccount.new(id: 1, color: color))
  end

  def event(id:, title:, start_at:)
    CalendarEvent.new(id: uuid_for(id), title: title, start_at: start_at, end_at: start_at + 3600,
                      all_day: false, status: :confirmed, calendar: calendar)
  end

  # A month where "today" carries more events than fit in a cell.
  def render_overflow_month
    base = Date.current.to_time
    titles = [ "Standup", "Design review", "1:1 with Sam", "Client call", "Lunch", "Retro" ]
    events = titles.each_with_index.map { |t, i| event(id: 70 + i, title: t, start_at: base.change(hour: 9 + i)) }
    ApplicationController.render(described_class.new(date: Date.current, events: events), layout: false)
  end

  it "creates events from an explicit add button, not a cell-wide click" do
    html = render_overflow_month

    # The add-event affordance is a labelled '+' that opens the new-event modal.
    expect(html).to include("aria-label=\"Add event\"")
    expect(html).to include("data-calendar-event-modal-open=\"/calendar_events/new?date=#{Date.current.iso8601}&view=month\"")

    # The whole-cell click-to-create is gone (no calendar-create controller, no
    # data-new-url on cells).
    expect(html).not_to include("data-new-url")
    expect(html).not_to include("calendar-create")
  end

  it "hides overflow behind an expandable day popover" do
    html = render_overflow_month

    # 6 events, 3 shown -> a "+3 more" trigger wired to the popover controller.
    expect(html).to include("calendar-day-popover")
    expect(html).to include("+3 more")
    expect(html).to include("data-action=\"calendar-day-popover#toggle\"")

    # The popover panel carries the full day — including events past the fold.
    expect(html).to include("Lunch")
    expect(html).to include("Retro")
    expect(html).to include("data-calendar-day-popover-target=\"panel\"")
  end

  it "renders a plain grid with no events" do
    html = ApplicationController.render(described_class.new(date: Date.current, events: []), layout: false)

    expect(html).to include("grid-cols-7")
    expect(html).not_to include("calendar-day-popover")
  end
end
