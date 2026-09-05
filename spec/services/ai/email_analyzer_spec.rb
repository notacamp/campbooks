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
end
