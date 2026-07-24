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

  # Temporal actionability: an email source carries a real received_at, so the builder
  # can tell a live ask from a long-dead one. A full resync of years-old mail must not
  # spray a flood of stale suggestions — but a genuinely-forgotten recent action should
  # still surface. Rule: keep if future-dated OR the source email is still recent.
  describe "temporal actionability (future OR recent source)" do
    def build_from(email, item)
      described_class.call(workspace: ws, source: email, raw_items: [ item ])
    end

    it "keeps a future-dated task even when the source email is a year old" do
      email = create(:email_message, received_at: 400.days.ago)
      item  = { "title" => "Renew the domain", "confidence" => 0.9, "due_date" => 60.days.from_now.to_date.iso8601 }
      expect(build_from(email, item).size).to eq(1)
    end

    it "drops a past-dated task from an old source email (a long-dead deadline)" do
      email = create(:email_message, received_at: 400.days.ago)
      item  = { "title" => "File Q2 2024 VAT", "confidence" => 0.9, "due_date" => 380.days.ago.to_date.iso8601 }
      expect(build_from(email, item)).to be_empty
    end

    it "drops an undated task from an old source email" do
      email = create(:email_message, received_at: 400.days.ago)
      item  = { "title" => "Reply about the proposal", "confidence" => 0.9 }
      expect(build_from(email, item)).to be_empty
    end

    it "keeps an undated task from a recent source email (a live ask)" do
      email = create(:email_message, received_at: 2.days.ago)
      item  = { "title" => "Send the signed contract", "confidence" => 0.9 }
      expect(build_from(email, item).size).to eq(1)
    end

    it "keeps a recently-overdue task so a forgotten action still surfaces" do
      email = create(:email_message, received_at: 5.days.ago)
      item  = { "title" => "Pay the invoice", "confidence" => 0.9, "due_date" => 2.days.ago.to_date.iso8601 }
      expect(build_from(email, item).size).to eq(1)
    end

    it "keeps a task dated today from an old email (today is still actionable)" do
      email = create(:email_message, received_at: 400.days.ago)
      item  = { "title" => "Call the notary", "confidence" => 0.9, "due_date" => Date.current.iso8601 }
      expect(build_from(email, item).size).to eq(1)
    end
  end

  it "the email-linking actions are registered" do
    expect(EmailActions.definition("create_task_from_email")).to be_truthy
    expect(EmailActions.definition("link_task_to_email")).to be_truthy
  end

  # ── Cross-source deterministic sibling (same date + title-key) ───────────────
  # Pin the clock so due-date arithmetic stays future-dated.
  describe "deterministic sibling dedup (same due date + title-key)" do
    around { |ex| travel_to(Time.zone.parse("2026-07-01 10:00:00")) { ex.run } }

    let(:ws5)    { Workspace.create!(name: "Task Sibling WS") }
    let(:user_a) { ws5.users.create!(name: "A", email_address: "a-task-sibling@example.com", password: "password123") }
    let(:user_b) { ws5.users.create!(name: "B", email_address: "b-task-sibling@example.com", password: "password123") }

    def build_task(source, title, extra = {})
      Tasks::Builder.call(
        workspace: ws5, source: source,
        raw_items: [ { "title" => title, "confidence" => 0.9, "due_date" => "2026-07-20" }.merge(extra) ],
        anchor_tz: Time.zone
      )
    end

    it "adopts an existing live task with the same due date and title-key from a different source" do
      first = build_task(user_a, "Submit the report").first
      expect(first).to be_persisted

      result = build_task(user_b, "Submit the report on 2026-07-20")
      expect(result.size).to eq(1)
      expect(result.first).to eq(first)
      expect(Task.where(workspace: ws5).count).to eq(1)
    end

    it "returns nil and does not create a new row when the sibling is done" do
      first = build_task(user_a, "Submit the report").first
      first.update!(status: :done)

      result = build_task(user_b, "Submit the report")
      expect(result).to eq([])
      expect(Task.where(workspace: ws5).count).to eq(1)
    end

    it "returns nil and does not create a new row when the sibling is cancelled" do
      first = build_task(user_a, "Submit the report").first
      first.update!(status: :cancelled)

      result = build_task(user_b, "Submit the report")
      expect(result).to eq([])
      expect(Task.where(workspace: ws5).count).to eq(1)
    end
  end

  # ── Novelty gate for tasks via Ai::CommitmentMatcher ────────────────────────
  describe "novelty gate via Ai::CommitmentMatcher" do
    around { |ex| travel_to(Time.zone.parse("2026-07-01 10:00:00")) { ex.run } }

    let(:ws6) { Workspace.create!(name: "Task Novelty WS") }
    let(:src) { ws6.users.create!(name: "Src", email_address: "src-tn@example.com", password: "password123") }

    let!(:existing_reminder) do
      Reminder.create!(
        workspace: ws6, source: src, reminder_type: :deadline,
        title: "Submit report", due_at: Time.zone.parse("2026-07-20 09:00:00"),
        status: :pending, confidence: 0.9
      )
    end

    def task_item
      { "title" => "Submit report", "confidence" => 0.9, "due_date" => "2026-07-20" }
    end

    it "does not create a task when matcher returns a Reminder" do
      matcher_double = instance_double(Ai::CommitmentMatcher, match: existing_reminder, failed?: false)
      allow(Ai::CommitmentMatcher).to receive(:new).and_return(matcher_double)

      result = Tasks::Builder.call(workspace: ws6, source: src, raw_items: [ task_item ], anchor_tz: Time.zone)
      expect(result).to eq([])
      expect(Task.where(workspace: ws6).count).to eq(0)
    end

    it "does not call CommitmentMatcher for a dateless item" do
      expect(Commitments::Neighbors).not_to receive(:around)

      Tasks::Builder.call(
        workspace: ws6, source: src,
        raw_items: [ { "title" => "Follow up", "confidence" => 0.9 } ],
        anchor_tz: Time.zone
      )
    end
  end
end
