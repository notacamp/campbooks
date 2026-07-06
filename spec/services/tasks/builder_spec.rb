require "rails_helper"

RSpec.describe Tasks::Builder do
  let(:ws) { Workspace.create!(name: "Builder WS") }
  # Any persisted record works as a polymorphic source for the builder's logic;
  # a user keeps the test free of email-account/document fixtures.
  let(:source) do
    ws.users.create!(name: "Src", email_address: "src-builder@example.com", password: "password123")
  end

  it "materializes a suggested, ai_suggested task from a raw item" do
    items = [ { "title" => "Sign the contract", "confidence" => 0.9, "priority" => "high", "due_date" => "2026-07-10" } ]

    tasks = described_class.call(workspace: ws, source: source, raw_items: items)

    expect(tasks.size).to eq(1)
    task = tasks.first
    expect(task).to be_suggested
    expect(task).to be_ai_suggested
    expect(task).to be_priority_high
    expect(task.title).to eq("Sign the contract")
    expect(task.source).to eq(source)
  end

  it "is idempotent across re-extraction of the same source + title" do
    items = [ { "title" => "Review proposal", "confidence" => 0.8 } ]
    described_class.call(workspace: ws, source: source, raw_items: items)

    expect {
      described_class.call(workspace: ws, source: source, raw_items: items)
    }.not_to change(Task, :count)
  end

  it "never overwrites a task the user already triaged" do
    items = [ { "title" => "Send invoice", "confidence" => 0.9 } ]
    task = described_class.call(workspace: ws, source: source, raw_items: items).first
    task.update!(status: :in_progress)

    # Same fingerprint (title is normalized for case/whitespace) — must not reset.
    described_class.call(workspace: ws, source: source, raw_items: [ { "title" => "  SEND INVOICE  ", "confidence" => 0.9 } ])

    expect(task.reload).to be_in_progress
  end

  it "drops items below the confidence floor" do
    items = [ { "title" => "Maybe do this", "confidence" => 0.2 } ]
    expect(described_class.call(workspace: ws, source: source, raw_items: items)).to be_empty
  end

  it "fingerprint_source collapses the same title across a thread's messages" do
    thread = ws.users.create!(name: "Thread", email_address: "thread-fp@example.com", password: "password123")
    other_message = ws.users.create!(name: "Msg2", email_address: "msg2-fp@example.com", password: "password123")
    items = [ { "title" => "Send the contract", "confidence" => 0.9 } ]

    first = described_class.call(workspace: ws, source: source, raw_items: items, fingerprint_source: thread).first

    expect {
      described_class.call(workspace: ws, source: other_message, raw_items: items, fingerprint_source: thread)
    }.not_to change(Task, :count)
    # The task still points at the message it came from, not the thread.
    expect(first.source).to eq(source)
  end

  it "keeps a past-due date — an overdue action still needs doing" do
    items = [ { "title" => "Overdue thing", "confidence" => 0.9, "due_date" => 3.days.ago.to_date.iso8601 } ]
    task = described_class.call(workspace: ws, source: source, raw_items: items).first

    expect(task.due_at).not_to be_nil
    expect(task.due_at).to be < Time.current
  end

  it "the email-linking actions are registered" do
    expect(EmailActions.definition("create_task_from_email")).to be_truthy
    expect(EmailActions.definition("link_task_to_email")).to be_truthy
  end
end
