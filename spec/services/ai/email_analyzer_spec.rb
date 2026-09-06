# frozen_string_literal: true

require "rails_helper"

RSpec.describe Ai::EmailAnalyzer do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace, email_address: "me@biz.example") }
  let(:contact)   { create(:contact, workspace: workspace, email_account: account,
                            email: "sender@x.example", sender_kind: :person) }
  let(:email) do
    create(:email_message, email_account: account, contact: contact,
           from_address: "sender@x.example", to_address: "me@biz.example",
           subject: "Q3 deck review", body: "Please review the attached slides.",
           received_at: 1.hour.ago)
  end

  let(:ai_response) do
    {
      "summary"           => "Sender requests review of Q3 slides.",
      "priority"          => "medium",
      "action_prompt"     => "Review the slides and flag any gaps.",
      "ask"               => "review of the Q3 slides",
      "suggested_actions" => [],
      "questions"         => []
    }.to_json
  end

  before do
    Current.workspace = workspace
    # Stub configuration so the analyzer uses an adapter path.
    config = { adapter: double(chat: ai_response), model: "test-model", temperature: 0.1 }
    allow(Ai::Configuration).to receive(:for_any).and_return(config)
    allow(Ai::Configuration).to receive(:user_prompt_suffix).and_return("")
    allow(Contacts::ContactContextBuilder).to receive(:new).and_return(double(context_for_prompt: ""))
  end

  after { Current.workspace = nil }

  describe "#analyze!" do
    it "updates ai_summary, ai_priority, ai_action_prompt, ai_ask and sets ai_analyzed_at" do
      described_class.new(email).analyze!
      email.reload

      expect(email.ai_summary).to include("Q3 slides")
      expect(email.ai_priority).to eq("medium")
      expect(email.ai_action_prompt).to include("slides")
      expect(email.ai_ask).to eq("review of the Q3 slides")
      expect(email.ai_analyzed_at).to be_within(5.seconds).of(Time.current)
    end

    it "truncates ai_ask to 120 chars" do
      long_ask = "a" * 200
      resp = { "summary" => "s", "priority" => "low", "action_prompt" => "",
               "ask" => long_ask, "suggested_actions" => [], "questions" => [] }.to_json
      config = { adapter: double(chat: resp), model: "test-model", temperature: 0.1 }
      allow(Ai::Configuration).to receive(:for_any).and_return(config)

      described_class.new(email).analyze!
      expect(email.reload.ai_ask.length).to be <= 120
    end

    it "skips already-analyzed emails" do
      email.update_columns(ai_analyzed_at: 1.hour.ago)
      adapter = instance_double("Ai::Adapters::Base", chat: "should not be called")
      config = { adapter: adapter, model: "test-model", temperature: 0.1 }
      allow(Ai::Configuration).to receive(:for_any).and_return(config)

      described_class.new(email).analyze!
      expect(adapter).not_to have_received(:chat)
    end

    it "skips security_flagged emails" do
      tag = create(:tag, workspace: workspace, name: "security_flagged")
      email.email_message_tags.create!(tag: tag)
      adapter = instance_double("Ai::Adapters::Base", chat: "should not be called")
      config = { adapter: adapter, model: "test-model", temperature: 0.1 }
      allow(Ai::Configuration).to receive(:for_any).and_return(config)

      described_class.new(email).analyze!
      expect(adapter).not_to have_received(:chat)
    end

    it "does not raise when adapter returns nil (graceful error path)" do
      config = { adapter: double(chat: nil), model: "test-model", temperature: 0.1 }
      allow(Ai::Configuration).to receive(:for_any).and_return(config)
      expect { described_class.new(email).analyze! }.not_to raise_error
      expect(email.reload.ai_analyzed_at).to be_nil
    end
  end

  # The analyzer's single read now also stages the asks it found (PR4): the work
  # Ai::TaskExtractor / Tasks::EmailExtractionJob used to do two minutes later.
  describe "#analyze! staging asks" do
    def stub_response(asks:)
      resp = {
        "summary" => "Sender requests review of Q3 slides.", "priority" => "medium",
        "action_prompt" => "", "ask" => "review of the Q3 slides",
        "suggested_actions" => [], "questions" => [], "asks" => asks
      }.to_json
      config = { adapter: double(chat: resp), model: "test-model", temperature: 0.1 }
      allow(Ai::Configuration).to receive(:for_any).and_return(config)
    end

    let(:one_ask) do
      [ { "title" => "Review the Q3 slides", "description" => "Look at slides 4 to 9 and reply with comments.",
          "due_date" => nil, "due_time" => nil, "priority" => "normal",
          "confidence" => 0.9, "justification" => '"Please review the attached slides."' } ]
    end

    it "persists the email fields AND stages a suggested Task through the builder" do
      stub_response(asks: one_ask)

      expect { described_class.new(email).analyze! }.to change { Task.count }.by(1)

      email.reload
      expect(email.ai_ask).to eq("review of the Q3 slides")
      task = Task.last
      expect(task).to be_suggested
      expect(task).to be_ai_suggested
      expect(task.title).to eq("Review the Q3 slides")
      expect(task.source).to eq(email)
      expect(task.workspace).to eq(workspace)
    end

    it "fingerprints on the email's thread so a restated ask collapses" do
      thread = create(:email_thread, email_account: account)
      email.update!(email_thread: thread)
      stub_response(asks: one_ask)

      described_class.new(email).analyze!

      expect(Task.last.extraction_fingerprint).to eq(
        Task.fingerprint_for(source_type: "EmailThread", source_id: thread.id, title: "Review the Q3 slides")
      )
    end

    it "drops an ask below the builder's confidence floor" do
      stub_response(asks: [ one_ask.first.merge("confidence" => 0.3) ])
      expect { described_class.new(email).analyze! }.not_to change { Task.count }
      expect(email.reload.ai_analyzed_at).to be_present
    end

    it "stages nothing for a vetoed (machine-category) email but still saves the analysis" do
      email.update_columns(category: "notifications")
      stub_response(asks: one_ask)

      expect { described_class.new(email).analyze! }.not_to change { Task.count }
      expect(email.reload.ai_summary).to be_present
    end

    it "stages nothing when the readiness flag is off" do
      allow(Features).to receive(:tasks?).and_return(false)
      stub_response(asks: one_ask)
      expect { described_class.new(email).analyze! }.not_to change { Task.count }
      expect(email.reload.ai_ask).to eq("review of the Q3 slides")
    end

    it "never lets a builder failure break the analysis (email fields already saved)" do
      stub_response(asks: one_ask)
      allow(Tasks::Builder).to receive(:call).and_raise(StandardError, "boom")

      expect { described_class.new(email).analyze! }.not_to raise_error
      expect(email.reload.ai_analyzed_at).to be_present
      expect(email.ai_ask).to eq("review of the Q3 slides")
    end

    it "includes the received date and tracked commitments in the prompt" do
      workspace.tasks.create!(title: "Send the signed NDA", status: :todo, priority: :normal,
                              due_at: 3.days.from_now)
      captured = nil
      adapter = double
      allow(adapter).to receive(:chat) do |**kw|
        captured = kw
        { "summary" => "s", "priority" => "low", "action_prompt" => "", "ask" => "",
          "suggested_actions" => [], "questions" => [], "asks" => [] }.to_json
      end
      allow(Ai::Configuration).to receive(:for_any).and_return(
        { adapter: adapter, model: "m", temperature: 0.0 }
      )

      described_class.new(email).analyze!

      content = captured[:messages].first[:content]
      expect(content).to include("<already_tracked_commitments>")
      expect(content).to include("Send the signed NDA")
      expect(content).to include("Received: #{email.received_at.to_date.iso8601}")
      expect(captured[:max_tokens]).to eq(900)
    end
  end
end
