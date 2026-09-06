# frozen_string_literal: true

require "rails_helper"

RSpec.describe Time::DayNote do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { allow(Features).to receive(:tasks?).and_return(true) }

  it "counts live undated asks without a held block" do
    workspace.tasks.create!(title: "A", status: :todo)
    workspace.tasks.create!(title: "B", status: :suggested, ai_suggested: true)
    held = workspace.tasks.create!(title: "Held", status: :todo)
    FocusBlock.create!(workspace: workspace, user: user, task: held, title: "Focus",
                       start_at: 1.day.from_now, end_at: 1.day.from_now + 45.minutes, status: :proposed)
    workspace.tasks.create!(title: "Dated", status: :todo, due_at: 2.days.from_now)
    workspace.tasks.create!(title: "Snoozed", status: :todo, snoozed_until: 1.week.from_now)

    result = described_class.for(user)
    expect(result.undated_count).to eq(2)
    expect(result.any?).to be(true)
  end

  it "names the ask, not the block's own title, as the held focus subject" do
    ask = workspace.tasks.create!(title: "Comments to Sofia", status: :todo, due_at: 3.days.from_now)
    FocusBlock.create!(workspace: workspace, user: user, task: ask, title: "Focus: Comments to Sofia",
                       start_at: 1.day.from_now, end_at: 1.day.from_now + 45.minutes, status: :kept)

    expect(described_class.for(user).focus.subject).to eq("Comments to Sofia")
  end

  it "reports zero undated asks when tasks are gated off" do
    allow(Features).to receive(:tasks?).and_return(false)
    workspace.tasks.create!(title: "A", status: :todo)

    expect(described_class.for(user).undated_count).to eq(0)
  end
end
