# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::TimePage::AgendaList, type: :component do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:zone) { user.effective_time_zone }

  def undated_item(task)
    Time::AgendaItem.new(
      kind: :task, at: nil, day: nil, all_day: true, overdue: false, duration_minutes: 0,
      title: task.title, source_label: "from Rita's email", source_path: nil, color: nil,
      record: task, actions: %i[schedule hold done]
    )
  end

  def render_list(**kwargs)
    ApplicationController.render(described_class.new(items: [], zone: zone, **kwargs), layout: false)
  end

  it "renders a No date yet section with the hint and the undated rows" do
    task = workspace.tasks.create!(title: "Send contract", status: :todo, priority: :normal)
    html = render_list(undated: [ undated_item(task) ])

    expect(html).to include("No date yet")
    expect(html).to include("Set a date, hold time, or finish it.")
    expect(html).to include("Send contract")
    expect(html).to include('id="time_agenda"')
  end

  it "omits the No date yet section when there are no undated asks" do
    html = render_list(undated: [])
    expect(html).not_to include("No date yet")
  end
end
