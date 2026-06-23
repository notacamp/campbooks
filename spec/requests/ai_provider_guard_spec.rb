require "rails_helper"

# AiProviderGuard stops an interactive AI action when the workspace has no provider
# for the capability it needs, and signals "set up AI" instead of silently no-opping
# (which would leave the user staring at a spinner). Scout's composer is the
# representative surface; the same guard protects compose, draft-reply, contacts, etc.
RSpec.describe "AI provider guard", type: :request do
  let(:user) { create(:user) }

  before { sign_in(user) }

  def post_to_scout
    post scout_path, params: { content: "summarize my inbox" }, as: :turbo_stream
  end

  context "when the text provider is unavailable" do
    before do
      allow(Ai::ProviderSetup).to receive(:available?).and_call_original
      allow(Ai::ProviderSetup).to receive(:available?).with(anything, :text).and_return(false)
    end

    it "does not enqueue Scout's reply and signals setup instead" do
      expect { post_to_scout }.not_to have_enqueued_job(AgentChatReplyJob)
      expect(response.body).to include("set up yet")
    end
  end

  context "when the text provider is available" do
    before do
      allow(Ai::ProviderSetup).to receive(:available?).and_call_original
      allow(Ai::ProviderSetup).to receive(:available?).with(anything, :text).and_return(true)
    end

    it "enqueues Scout's reply" do
      expect { post_to_scout }.to have_enqueued_job(AgentChatReplyJob)
    end
  end
end
