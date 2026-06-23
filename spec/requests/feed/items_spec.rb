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
end
