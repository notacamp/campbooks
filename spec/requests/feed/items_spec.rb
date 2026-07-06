require "rails_helper"

RSpec.describe "Feed::Items", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  let(:turbo) { { "Accept" => "text/vnd.turbo-stream.html" } }

  before { create(:email_account_user, user: user, email_account: account) }

  def feed_item_for(subject, kind:, **overrides)
    FeedItem.create!({ user: user, workspace: workspace, kind: kind, subject: subject,
                       dedupe_key: "#{kind}:#{subject.id}", sort_at: Time.current,
                       generated_at: Time.current }.merge(overrides))
  end

  context "when signed in" do
    before { sign_in(user) }

    it "acts on an email card through the EmailActions registry" do
      message = create(:email_message, email_account: account, ai_action_prompt: "Reply")
      item = feed_item_for(message, kind: "email_action")
      allow(EmailActions).to receive(:run)
        .and_return({ success: true, tool: "archive", message: "Archived", result: {} })

      post act_feed_item_path(item), params: { tool: "archive" }, headers: turbo

      expect(EmailActions).to have_received(:run).with("archive", hash_including(email_message: message, user: user))
      expect(item.reload.acted_at).to be_present
    end

    it "dismisses a card without touching the underlying record" do
      message = create(:email_message, email_account: account, ai_action_prompt: "Reply")
      item = feed_item_for(message, kind: "email_action")

      post dismiss_feed_item_path(item), headers: turbo

      expect(response).to have_http_status(:ok)
      expect(item.reload.dismissed_at).to be_present
      expect(message.reload.skimmed_at).to be_nil
    end

    it "marks a card seen" do
      message = create(:email_message, email_account: account, ai_action_prompt: "Reply")
      item = feed_item_for(message, kind: "email_action")

      post seen_feed_item_path(item)

      expect(response).to have_http_status(:no_content)
      expect(item.reload.seen_at).to be_present
    end

    it "offers an Undo toast for a reversible action and restores it on undo" do
      message = create(:email_message, email_account: account, ai_action_prompt: "File")
      item = feed_item_for(message, kind: "email_action")
      allow(EmailActions).to receive(:run)
        .and_return({ success: true, tool: "add_tag", message: "Filed", result: {} })

      post act_feed_item_path(item), params: { tool: "add_tag", args: { tag_name: "invoices" } }, headers: turbo
      expect(item.reload.acted_at).to be_present
      expect(response.body).to include(undo_feed_item_path(item)) # Undo affordance wired

      post undo_feed_item_path(item), params: { tool: "add_tag", args: { tag_name: "invoices" } }, headers: turbo
      expect(response).to have_http_status(:ok)
      expect(item.reload).to be_active             # card re-activated
      expect(response.body).to include("feed_item_#{item.id}") # card re-injected
    end

    it "undo of a dismissal re-activates the card without touching the record" do
      message = create(:email_message, email_account: account, ai_action_prompt: "Reply")
      item = feed_item_for(message, kind: "email_action")

      post dismiss_feed_item_path(item), headers: turbo
      expect(item.reload).not_to be_active

      post undo_feed_item_path(item), params: { tool: "dismiss_card" }, headers: turbo
      expect(item.reload).to be_active
    end

    context "tag-suggestion learning" do
      let(:contact) { create(:contact, workspace: workspace) }

      def tag_card(tag: "invoices")
        message = create(:email_message, email_account: account, from_address: "billing@acme.com", contact_id: contact.id)
        feed_item_for(message, kind: "tag_suggestion", data: { "tag_name" => tag })
      end

      it "records an accepted decision when a tag suggestion is acted on" do
        item = tag_card
        allow(EmailActions).to receive(:run).and_return({ success: true, tool: "add_tag", message: "Filed", result: {} })

        expect {
          post act_feed_item_path(item), params: { tool: "add_tag", args: { tag_name: "invoices" } }, headers: turbo
        }.to change { LearningDecision.where(domain: "tag_suggestion", label: "accepted", user: user).count }.by(1)

        decision = LearningDecision.find_by(domain: "tag_suggestion", user: user)
        expect(decision).to have_attributes(contact_id: contact.id, sender_domain: "acme.com")
        expect(decision.signals["tag_name"]).to eq("invoices")
      end

      it "records a rejected decision when a tag suggestion is dismissed" do
        item = tag_card(tag: "promotions")

        expect {
          post dismiss_feed_item_path(item), headers: turbo
        }.to change { LearningDecision.where(domain: "tag_suggestion", label: "rejected", user: user).count }.by(1)
      end

      it "does not record a decision for a non-tag-suggestion card" do
        message = create(:email_message, email_account: account, ai_action_prompt: "Reply")
        item = feed_item_for(message, kind: "email_action")

        expect { post dismiss_feed_item_path(item), headers: turbo }.not_to change(LearningDecision, :count)
      end
    end
  end

  it "404s an item that belongs to another user (no existence leak)" do
    message = create(:email_message, email_account: account, ai_action_prompt: "Reply")
    item = feed_item_for(message, kind: "email_action")

    other = create(:user)
    sign_in(other)
    post dismiss_feed_item_path(item), headers: turbo

    expect(response).to have_http_status(:not_found)
    expect(item.reload.dismissed_at).to be_nil
  end

  # -- task feed items (from feed/items_controller_test.rb) -------------------

  context "with task feed items" do
    let(:task) do
      Task.create!(
        workspace: workspace, title: "Send the signed statements",
        status: :suggested, priority: :normal, ai_suggested: true, confidence: 0.9
      )
    end
    let(:task_item) do
      FeedItem.create!(
        user: user, workspace: workspace, kind: "task", subject: task,
        dedupe_key: "task_suggestion:#{task.id}", sort_at: Time.current
      )
    end

    before { sign_in(user) }

    it "accept promotes the suggestion to todo and resolves the card" do
      post act_feed_item_path(task_item, format: :turbo_stream), params: { tool: "accept" }

      expect(response).to have_http_status(:ok)
      expect(task.reload.todo?).to be true
      expect(task_item.reload.acted?).to be true
    end

    it "dismiss_task cancels the suggestion" do
      post act_feed_item_path(task_item, format: :turbo_stream), params: { tool: "dismiss_task" }

      expect(response).to have_http_status(:ok)
      expect(task.reload.cancelled?).to be true
      expect(task_item.reload.acted?).to be true
    end

    it "another user's task item 404s" do
      other = workspace.users.create!(
        name: "Other",
        email_address: "other-#{SecureRandom.hex(4)}@example.com",
        password: "password123"
      )
      foreign = FeedItem.create!(
        user: other, workspace: workspace, kind: "task", subject: task,
        dedupe_key: "task_suggestion:other", sort_at: Time.current
      )

      post act_feed_item_path(foreign, format: :turbo_stream), params: { tool: "accept" }

      expect(response).to have_http_status(:not_found)
      expect(task.reload.suggested?).to be true
    end
  end
end
