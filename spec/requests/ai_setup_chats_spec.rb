require "rails_helper"

RSpec.describe "AiSetupChats", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  before { sign_in(user) }

  describe "POST /ai_setup/:kind (create)" do
    it "starts a thread and enqueues the first reply when AI is available" do
      allow_any_instance_of(Ai::OnboardingAssistant).to receive(:available?).and_return(true)

      expect {
        post start_ai_setup_chat_path(kind: "document_types")
      }.to have_enqueued_job(AiSetupChatReplyJob)

      thread = workspace.agent_threads.find_by(purpose: :setup_chat, title: "setup_document_types")
      expect(thread).to be_present
      expect(thread.agent_messages.count).to eq(1)
      expect(response.body).to include("Type your answer") # the live panel rendered
    end

    it "renders the unavailable frame (no thread) when AI is not configured" do
      allow_any_instance_of(Ai::OnboardingAssistant).to receive(:available?).and_return(false)

      post start_ai_setup_chat_path(kind: "tags")

      expect(response.body).to include("Set up your AI provider")
      expect(workspace.agent_threads.count).to eq(0)
    end

    # Regression: configuring AI for any text purpose (the quick modal sets up
    # email_classification) must make the setup chat available — it used to insist
    # on global_chat specifically, so this step re-prompted "needs an AI provider".
    it "is available once any text purpose is configured, not just global_chat" do
      adapter = workspace.ai_adapters.create!(name: "Text", provider: "deepseek", api_key: "k", enabled: true)
      workspace.ai_configurations.create!(
        purpose: "email_classification", ai_adapter: adapter,
        model: "deepseek-v4-pro", max_tokens: 1000, temperature: 0.0, enabled: true
      )

      expect { post start_ai_setup_chat_path(kind: "document_types") }
        .to have_enqueued_job(AiSetupChatReplyJob)
      expect(response.body).not_to include("Set up your AI provider")
    end

    it "404s for an unknown kind" do
      post start_ai_setup_chat_path(kind: "bogus")
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "POST /ai_setup/:kind/message" do
    it "records the answer, streams it back, and enqueues the reply" do
      expect {
        post ai_setup_chat_message_path(kind: "tags"), params: { content: "we run a bakery" }
      }.to have_enqueued_job(AiSetupChatReplyJob)

      expect(response.media_type).to eq("text/vnd.turbo-stream.html")
      expect(response.body).to include("we run a bakery")
    end
  end

  describe "GET /ai_setup/:kind (show) with a pending proposal" do
    it "renders the proposal component" do
      thread = workspace.agent_threads.create!(purpose: :setup_chat, title: "setup_document_types", user: user)
      thread.agent_messages.create!(content: "I'm ready", author_type: :user, user: user)
      thread.agent_messages.create!(
        content: "Here's what I suggest", author_type: :ai, user: user,
        ai_suggested_actions: [ { "name" => "invoice", "color" => "#3b82f6", "prompt" => "Vendor invoices." } ]
      )

      get ai_setup_chat_path(kind: "document_types"), headers: { "Turbo-Frame" => "setup_modal_frame" }

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Add selected")
      expect(response.body).to include("Invoice")
    end
  end

  describe "POST /ai_setup/:kind/apply" do
    it "persists the selected suggestions and closes the dialog" do
      thread = workspace.agent_threads.create!(purpose: :setup_chat, title: "setup_tags", user: user)
      thread.agent_messages.create!(
        content: "proposal", author_type: :ai, user: user,
        ai_suggested_actions: [ { "name" => "urgent", "color" => "#ef4444", "prompt" => "x" } ]
      )

      expect {
        post ai_setup_chat_apply_path(kind: "tags"),
             params: { items: { "0" => { "selected" => "1", "name" => "urgent", "color" => "#ef4444", "prompt" => "x" } } }
      }.to change { workspace.tags.count }.by(1)

      expect(response.body).to include("setup_banner")     # banner refreshed
      expect(response.body).to include("dialog-close")      # dialog closed
    end
  end
end
