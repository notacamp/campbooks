require "rails_helper"

RSpec.describe Task do
  before do
    @ws = Workspace.create!(name: "Tasks Test WS")
    @user = @ws.users.create!(name: "Tess", email_address: "tess-tasks@example.com", password: "password123")
  end

  it "requires a title" do
    task = @ws.tasks.build(status: :todo, priority: :normal)
    expect(task).not_to be_valid
    expect(task.errors[:title]).to include("can't be blank")
  end

  it "status defaults to suggested and priority to normal" do
    task = @ws.tasks.create!(title: "Triage me")
    expect(task.status).to eq("suggested")
    expect(task).to be_priority_normal
  end

  it "fingerprint_for is deterministic and normalizes the title" do
    a = Task.fingerprint_for(source_type: "EmailMessage", source_id: "abc", title: "Pay Invoice")
    b = Task.fingerprint_for(source_type: "EmailMessage", source_id: "abc", title: "  pay invoice  ")
    expect(b).to eq(a)
  end

  it "accessible_to is workspace-scoped and fails closed for a nil user" do
    mine   = @ws.tasks.create!(title: "Mine")
    theirs = Workspace.create!(name: "Other WS").tasks.create!(title: "Theirs")

    expect(Task.accessible_to(@user)).to include(mine)
    expect(Task.accessible_to(@user)).not_to include(theirs)
    expect(Task.accessible_to(nil)).to be_empty
  end

  it "move_to_status! to done stamps completed_at and logs events" do
    task = @ws.tasks.create!(title: "Ship it", status: :todo, created_by: @user)

    task.move_to_status!(:done, by: @user)

    expect(task).to be_done
    expect(task.completed_at).not_to be_nil
    names = Event.for_subject(task).pluck(:name)
    expect(names).to include("task.status_changed")
    expect(names).to include("task.completed")
  end

  it "move_to_status! away from done clears completed_at" do
    task = @ws.tasks.create!(title: "Reopen", status: :done, completed_at: Time.current, created_by: @user)

    task.move_to_status!(:in_progress, by: @user)

    expect(task).to be_in_progress
    expect(task.completed_at).to be_nil
  end

  it "move_to_status! rejects an unknown status" do
    task = @ws.tasks.create!(title: "Nope", status: :todo)
    expect { task.move_to_status!(:nonsense) }.to raise_error(ArgumentError)
  end

  it "overdue scope returns only active, past-due tasks" do
    overdue   = @ws.tasks.create!(title: "late", status: :todo, due_at: 1.day.ago)
    upcoming  = @ws.tasks.create!(title: "soon", status: :todo, due_at: 1.day.from_now)
    done_late = @ws.tasks.create!(title: "done late", status: :done, due_at: 1.day.ago)

    expect(Task.overdue).to include(overdue)
    expect(Task.overdue).not_to include(upcoming)
    expect(Task.overdue).not_to include(done_late)
  end

  it "shares the Tag model with emails through task_tags" do
    tag  = Tag.create!(workspace: @ws, name: "finance", color: "#6366f1")
    task = @ws.tasks.create!(title: "Budget")
    task.tags << tag

    expect(task.reload.tags).to include(tag)
    expect(tag.reload.tasks).to include(task)
  end
end
