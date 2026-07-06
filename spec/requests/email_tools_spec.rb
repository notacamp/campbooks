require "rails_helper"

# The Scout suggested-action endpoint, now dispatching through EmailActions.
RSpec.describe "Email tools (Scout suggested actions)", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:message) { create(:email_message, email_account: account) }
  let!(:tag) { workspace.tags.create!(name: "invoice", color: "#2563eb") }

  before do
    create(:email_account_user, :collaborator, user: user, email_account: account)
    sign_in(user)
  end

  describe "IDOR: complete_suggested_action with another user's AgentMessage" do
    let(:other_workspace) { create(:workspace) }
    let(:other_user) { create(:user, workspace: other_workspace) }
    let(:other_thread) { create(:agent_thread, user: other_user, workspace: other_workspace) }
    let!(:victim_message) do
      create(:agent_message,
        user: other_user,
        agent_thread: other_thread,
        content: "Scout reply",
        author_type: :ai,
        ai_suggested_actions: [ { "tool" => "add_tag", "args" => {} } ],
        ai_auto_actions: [])
    end

    it "does not update another user's AgentMessage when agent_message_id is supplied" do
      post tool_email_message_path(message),
        params: { tool: "add_tag", args: { tag_name: "invoice" }, agent_message_id: victim_message.id },
        as: :turbo_stream

      expect(response).to have_http_status(:ok)
      victim_message.reload
      # The victim's ai_auto_actions must remain empty — the action was scoped
      # away because the AgentMessage belongs to a different user.
      expect(victim_message.ai_auto_actions).to be_empty
    end
  end

  it "applies an existing tag" do
    post tool_email_message_path(message), params: { tool: "add_tag", args: { tag_name: "invoice" } }, as: :turbo_stream

    expect(response).to have_http_status(:ok)
    expect(message.reload.tags.map(&:name)).to include("invoice")
  end

  it "returns a specific error (not a generic 'Action failed') for a tag that doesn't exist" do
    post tool_email_message_path(message), params: { tool: "add_tag", args: { tag_name: "ghost" } }, as: :turbo_stream

    expect(response).to have_http_status(:unprocessable_entity)
    expect(response.body).to match(/no tag named/i)
    expect(response.body).to include("ghost")
  end

  # Rewind highlight cards (home feed, "Looking back") archive straight to the
  # email — they aren't materialized feed items — so the turbo response must
  # target the card by its own id, not the inbox nodes.
  describe "archive from a Rewind highlight" do
    before { allow(Tools::Archive).to receive(:call).and_return(true) }

    it "removes the highlight card by id and offers an undo carrying surface + reason" do
      post tool_email_message_path(message, surface: "rewind", reason: "starred"),
        params: { tool: "archive" }, as: :turbo_stream

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("rewind_highlight_#{message.id}") # remove target
      expect(response.body).to include("remove")
      expect(response.body).to include("unarchive") # undo re-runs unarchive
      expect(response.body).to include("starred")   # reason carried for re-insert
    end
  end

  describe "unarchive (undo) from a Rewind highlight" do
    before { allow(Tools::Unarchive).to receive(:call).and_return(true) }

    it "re-inserts the highlight card into the feed timeline" do
      post tool_email_message_path(message, surface: "rewind", reason: "starred"),
        params: { tool: "unarchive" }, as: :turbo_stream

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("feed_timeline")                  # prepend target
      expect(response.body).to include("rewind_highlight_#{message.id}") # re-rendered card
    end
  end

  describe "draft_reply" do
    let(:thread) { create(:email_thread, email_account: account) }
    let(:message) { create(:email_message, email_account: account, email_thread: thread) }

    before do
      # Bypass the AI-provider gate and the live model call.
      allow_any_instance_of(EmailToolsController).to receive(:require_ai_provider!).and_return(false)
      allow(Tools::DraftReply).to receive(:call).and_return(draft: { "subject" => "Re: Hi", "body" => "Sure thing — G" })
    end

    it "renders the editable preview into the surface's compose slot" do
      post tool_email_message_path(message, surface: "detail"), params: { tool: "draft_reply" }, as: :turbo_stream
      expect(response).to have_http_status(:ok)
      # The compose slot moved from thread_compose_target to the global compose_dock
      # (dock_with_scout_stream — called when surface="detail" has a slot defined).
      expect(response.body).to include("compose_dock")
      expect(response.body).to include("Sure thing")
    end

    # Regression: AI draft messages must carry a user (AgentMessage belongs_to
    # :user is required) — otherwise the whole action 422s.
    it "persists the AI draft as an AgentMessage with a user" do
      expect {
        post tool_email_message_path(message, surface: "detail"), params: { tool: "draft_reply" }, as: :turbo_stream
      }.to change(AgentMessage.where(author_type: :ai, draft: true), :count).by(1)
      expect(AgentMessage.where(author_type: :ai).last.user).to eq(user)
    end
  end

  describe "send_reply (inline Scout reply) flips the CTA to follow-up" do
    let(:thread) { create(:email_thread, email_account: account) }
    let(:replied_message) { create(:email_message, email_account: account, email_thread: thread) }

    before do
      # Stub the provider so nothing is actually sent.
      client = double("mail_client", save_draft: { "id" => "draft-1" }, send_draft: true)
      allow_any_instance_of(EmailAccount).to receive(:mail_client).and_return(client)
    end

    it "marks the thread answered and re-renders the Scout strip as Draft follow-up" do
      post tool_email_message_path(replied_message, surface: "detail"),
           params: { tool: "send_reply", body: "On it — sending now." }, as: :turbo_stream

      expect(response).to have_http_status(:ok)
      expect(thread.reload.last_outbound_at).to be_present
      expect(response.body).to include("scout_actions_#{replied_message.id}")
      expect(response.body).to include("Draft follow-up")
      expect(response.body).not_to include("Suggest reply")
    end
  end
end
