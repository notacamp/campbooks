# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::TimePage::AgendaRow, type: :component do
  include Rails.application.routes.url_helpers

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:zone) { user.effective_time_zone }

  def agenda_item(task, **overrides)
    Time::AgendaItem.new(**{
      kind: :task, at: nil, day: nil, all_day: true, overdue: false, duration_minutes: 0,
      title: task.title, source_label: "from Rita's email", source_path: nil, color: nil,
      record: task, actions: []
    }.merge(overrides))
  end

  def render_row(item, **kwargs)
    ApplicationController.render(described_class.new(item: item, zone: zone, **kwargs), layout: false)
  end

  it "renders an undated ask row with Set a date, Hold, Done and a kebab" do
    task = workspace.tasks.create!(title: "Send contract", status: :suggested, priority: :normal)
    item = agenda_item(task, actions: %i[schedule hold done snooze dismiss_ask])
    html = render_row(item, hold_slot: 1.day.from_now.change(hour: 10))

    expect(html).to include("no date")           # time cell
    expect(html).to include("Set a date")
    expect(html).to include("Hold")
    expect(html).to include("Done")
    expect(html).to include(schedule_ask_path(task))
    expect(html).to include(hold_ask_path(task))
    expect(html).to include("Not now")           # kebab snooze
    expect(html).to include("Dismiss")           # kebab dismiss_ask
    expect(html).to include("ask")               # meta lead
  end

  it "renders a dated ask kebab with Change date and a hold entry" do
    task = workspace.tasks.create!(title: "Comments", status: :todo, priority: :normal, due_at: 2.days.from_now)
    item = agenda_item(task, at: task.due_at, day: task.due_at.to_date, all_day: false,
                       actions: %i[done change_date hold snooze])
    html = render_row(item, hold_slot: 1.day.from_now.change(hour: 10))

    expect(html).to include("Change date")
    expect(html).to include("Done")
    expect(html).to include(hold_ask_path(task)) # hold offered in the kebab
    expect(html).not_to include("no date")       # dated row shows a time, not "no date"
  end
end
