# frozen_string_literal: true

require "rails_helper"

RSpec.describe Time::FocusProposer do
  around { |ex| travel_to(Time.utc(2026, 9, 7, 8, 0, 0)) { ex.run } } # Monday

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before { allow(Features).to receive(:tasks?).and_return(true) }

  def ask(**attrs)
    workspace.tasks.create!({ title: "Ask", status: :todo, priority: :normal }.merge(attrs))
  end

  it "proposes a focus block for an accepted dated ask within 7 days" do
    task = ask(due_at: 3.days.from_now.change(hour: 17))

    expect { described_class.for(user) }.to change { FocusBlock.for_task(task).count }.by(1)
    block = FocusBlock.for_task(task).first
    expect(block.reason).to start_with("ask_due_")
    expect(block).to be_proposed
  end

  it "never proposes for a suggested ask" do
    task = ask(status: :suggested, ai_suggested: true, due_at: 3.days.from_now.change(hour: 17))
    expect { described_class.for(user) }.not_to change { FocusBlock.for_task(task).count }
  end

  it "never proposes for an undated ask" do
    task = ask(due_at: nil)
    expect { described_class.for(user) }.not_to change { FocusBlock.for_task(task).count }
  end

  it "does not propose for an ask beyond the 7-day lookahead" do
    task = ask(due_at: 20.days.from_now.change(hour: 17))
    expect { described_class.for(user) }.not_to change { FocusBlock.for_task(task).count }
  end

  it "never proposes twice for the same ask (one block per ask ever)" do
    task = ask(due_at: 3.days.from_now.change(hour: 17))
    described_class.for(user)
    expect { described_class.for(user) }.not_to change { FocusBlock.for_task(task).count }
  end

  it "still proposes for eligible deadline reminders (unchanged)" do
    reminder = create(:reminder, workspace: workspace, reminder_type: :deadline,
                      due_at: 3.days.from_now.change(hour: 17), status: :pending)

    expect { described_class.for(user) }.to change { FocusBlock.where(reminder: reminder).count }.by(1)
  end

  it "skips the ask leg entirely when Features.tasks? is off" do
    allow(Features).to receive(:tasks?).and_return(false)
    task = ask(due_at: 3.days.from_now.change(hour: 17))
    expect { described_class.for(user) }.not_to change { FocusBlock.for_task(task).count }
  end
end
