# frozen_string_literal: true

require "rails_helper"

RSpec.describe Emails::ScoutDraft do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:thread) { create(:email_thread, email_account: account) }
  let(:message) { create(:email_message, email_account: account, email_thread: thread) }

  describe ".for" do
    context "when there is no agent thread" do
      it "returns nil" do
        expect(described_class.for(message)).to be_nil
      end
    end

    context "when there is an agent thread" do
      let(:agent_thread) do
        create(:agent_thread, user: user, workspace: workspace,
               contextable: thread, purpose: :email_chat)
      end

      it "returns nil when there are no draft messages" do
        agent_thread
        expect(described_class.for(message)).to be_nil
      end

      it "returns the content of the freshest non-outdated AI draft" do
        agent_thread
        create(:agent_message, agent_thread: agent_thread, user: user,
               author_type: :ai, draft: true, outdated: false,
               ai_suggested_actions: [], content: "Friday works for me.")
        expect(described_class.for(message)).to eq("Friday works for me.")
      end

      it "returns nil when the only draft is outdated" do
        agent_thread
        create(:agent_message, agent_thread: agent_thread, user: user,
               author_type: :ai, draft: true, outdated: true,
               ai_suggested_actions: [], content: "Old draft.")
        expect(described_class.for(message)).to be_nil
      end

      it "returns nil when the only draft carries suggested actions (question prompt)" do
        agent_thread
        create(:agent_message, agent_thread: agent_thread, user: user,
               author_type: :ai, draft: true, outdated: false,
               ai_suggested_actions: [ { "question" => "When?" } ], content: "What date?")
        expect(described_class.for(message)).to be_nil
      end

      it "returns the newest draft when multiple exist" do
        agent_thread
        create(:agent_message, agent_thread: agent_thread, user: user,
               author_type: :ai, draft: true, outdated: false,
               ai_suggested_actions: [], content: "Older draft.", created_at: 2.hours.ago)
        create(:agent_message, agent_thread: agent_thread, user: user,
               author_type: :ai, draft: true, outdated: false,
               ai_suggested_actions: [], content: "Newer draft.", created_at: 1.hour.ago)
        expect(described_class.for(message)).to eq("Newer draft.")
      end

      it "returns nil for a user (not AI) draft" do
        agent_thread
        create(:agent_message, agent_thread: agent_thread, user: user,
               author_type: :user, draft: true, outdated: false,
               ai_suggested_actions: [], content: "My own draft.")
        expect(described_class.for(message)).to be_nil
      end
    end

    context "with a nil message" do
      it "returns nil" do
        expect(described_class.for(nil)).to be_nil
      end
    end
  end
end
