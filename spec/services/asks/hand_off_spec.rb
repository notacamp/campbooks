# frozen_string_literal: true

require "rails_helper"

RSpec.describe Asks::HandOff do
  let(:workspace) { create(:workspace) }
  let(:assigner) { create(:user, workspace: workspace, name: "Guilherme A") }
  let(:assignee) { create(:user, workspace: workspace, name: "Ana Ng") }

  def ask(**attrs)
    workspace.tasks.create!({ title: "Countersign Acme", status: :suggested, priority: :normal }.merge(attrs))
  end

  it "replaces the assignments with a single one and accepts a suggested ask" do
    task = ask(status: :suggested)
    task.task_assignments.create!(user: assigner, assigned_by: assigner) # a prior assignment

    described_class.call(task, to: assignee, by: assigner)

    task.reload
    expect(task.assignees).to contain_exactly(assignee)
    expect(task.handed_by).to eq(assigner)
    expect(task).to be_todo # suggested → accepted on hand-off
  end

  it "publishes task.handed_off with the payload" do
    task = ask(status: :todo)
    described_class.call(task, to: assignee, by: assigner)

    event = Event.for_subject(task).find_by(name: "task.handed_off")
    expect(event).to be_present
    expect(event.payload["to_user_id"]).to eq(assignee.id)
    expect(event.payload["by_user_id"]).to eq(assigner.id)
  end

  it "notifies the assignee with an action_required notice linking to Time" do
    task = ask(status: :todo)
    described_class.call(task, to: assignee, by: assigner)

    notif = assignee.notifications.last
    expect(notif.category).to eq("task")
    expect(notif.priority).to eq("action_required")
    expect(notif.link_url).to eq("/time")
    expect(notif.notifiable).to eq(task)
  end

  it "resolves the assignee's notice when the ask is completed" do
    task = ask(status: :todo)
    described_class.call(task, to: assignee, by: assigner)
    notif = assignee.notifications.last
    expect(notif.active?).to be(true)

    task.move_to_status!(:done, by: assignee)

    expect(notif.reload.active?).to be(false)
  end
end
