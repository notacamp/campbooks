require "rails_helper"

RSpec.describe Reminders::EmailExtractionJob do
  let(:email) { create(:email_message) }

  it "skips extraction when the gate rejects the email" do
    allow(Reminders::ExtractionGate).to receive(:email_allows?).and_return(false)
    expect(Ai::ReminderExtractor).not_to receive(:new)
    expect(Reminders::Builder).not_to receive(:call)
    described_class.new.perform(email.id)
  end

  it "extracts and builds when the gate passes" do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
    allow(Reminders::ExtractionGate).to receive(:email_allows?).and_return(true)
    allow_any_instance_of(Ai::ReminderExtractor).to receive(:extract).and_return([ { "reminder_type" => "deadline" } ])

    # previously_new_record? false ⇒ the announcement step skips it, keeping this
    # example focused on the extract/build pipeline (announcement is covered by the
    # job's own unit test).
    expect(Reminders::Builder).to receive(:call)
      .with(hash_including(source: email))
      .and_return([ instance_double(Reminder, previously_new_record?: false) ])

    described_class.new.perform(email.id)
  end

  it "skips extraction when no text AI provider is configured" do
    allow(Reminders::ExtractionGate).to receive(:email_allows?).and_return(true)
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(false)

    expect(Ai::ReminderExtractor).not_to receive(:new)
    expect(Reminders::Builder).not_to receive(:call)

    described_class.new.perform(email.id)
  end

  it "no-ops for a missing email" do
    expect { described_class.new.perform(-1) }.not_to raise_error
  end

  it "passes known_commitments and tasks_active to the extractor" do
    allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
    allow(Reminders::ExtractionGate).to receive(:email_allows?).and_return(true)
    allow(Commitments::Known).to receive(:for).and_return([ "- [task] Do something — due 2026-07-30" ])

    expect(Ai::ReminderExtractor).to receive(:new).with(
      hash_including(
        known_commitments: [ "- [task] Do something — due 2026-07-30" ],
        tasks_active: (Features.tasks? && email.email_account.workspace.entitlements.feature?(:tasks))
      )
    ).and_return(double(extract: []))

    allow(Reminders::Builder).to receive(:call).and_return([])
    described_class.new.perform(email.id)
  end

  # ── announce_in_discussion: focused on the discussion-announcement selection ──
  # Focused on the discussion-announcement selection added to the job; the AI
  # extraction itself is exercised by the extractor/builder specs.
  describe "#announce_in_discussion" do
    let(:workspace) { Workspace.create!(name: "Reminder Job WS") }
    let(:owner) do
      User.create!(
        workspace: workspace, email_address: "owner@example.com",
        name: "owner", password: "password123", password_confirmation: "password123"
      )
    end
    let(:account) do
      acct = EmailAccount.create!(
        workspace: workspace, email_address: "mailbox@example.com",
        provider: :google, refresh_token: "tok", active: true
      )
      acct.email_account_users.create!(user: owner, owner: true, can_read: true)
      acct
    end
    let(:thread) { account.email_threads.create!(subject: "Invoice 1234") }
    let(:message) do
      account.email_messages.create!(
        email_thread: thread, provider_message_id: "m-1", provider_folder_id: "INBOX",
        from_address: "billing@acme.test", to_address: "mailbox@example.com",
        subject: "Invoice 1234", received_at: Time.current, read: false, has_attachment: false
      )
    end

    def run_announce(reminders)
      described_class.new.send(:announce_in_discussion, message, reminders)
    end

    def build_reminder(title:, confidence:, type: :deadline)
      Reminder.create!(
        workspace: workspace, source: message, reminder_type: type,
        title: title, due_at: 3.days.from_now, status: :pending, confidence: confidence
      )
    end

    it "posts one approvable suggestion message per confident, newly-created reminder" do
      r1 = build_reminder(title: "Pay invoice 1234", confidence: 0.9)
      r2 = build_reminder(title: "Renew the plan", confidence: 0.7, type: :renewal)

      # One card each, so every reminder can be approved or dismissed on its own.
      expect { run_announce([ r1, r2 ]) }.to change(AgentMessage, :count).by(2)

      messages = thread.reload.agent_thread.agent_messages.order(:created_at).to_a
      contents = messages.map(&:content)
      expect(contents.all? { |c| c.include?("potential reminder") }).to be true
      expect(contents.any? { |c| c.include?("Pay invoice 1234") && c.include?("/reminders#reminder_#{r1.id}") }).to be true
      expect(contents.any? { |c| c.include?("Renew the plan") && c.include?("/reminders#reminder_#{r2.id}") }).to be true

      # Each card carries inline Approve/Dismiss actions targeting its own reminder.
      card = messages.find { |m| m.content.include?("Pay invoice 1234") }
      expect(card.ai_suggested_actions.map { |a| a["tool"] }.sort).to eq(%w[confirm_reminder dismiss_reminder])
      expect(card.ai_suggested_actions.all? { |a| a["args"]["reminder_id"] == r1.id }).to be true
    end

    it "skips reminders below the confidence floor" do
      low = build_reminder(title: "Maybe a thing", confidence: 0.55)

      expect { run_announce([ low ]) }.not_to change(AgentMessage, :count)
      expect(thread.reload.agent_thread).to be_nil
    end

    it "skips reminders the builder only re-touched (not newly created this run)" do
      existing = build_reminder(title: "Already known", confidence: 0.9)
      existing = Reminder.find(existing.id) # re-load ⇒ previously_new_record? is false

      expect { run_announce([ existing ]) }.not_to change(AgentMessage, :count)
    end
  end
end
