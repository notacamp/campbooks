require "rails_helper"

RSpec.describe Feed::RefreshJob do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account)
    allow(Feed::LiveDeck).to receive(:broadcast)
  end

  it "regenerates the feed and streams the newly inserted cards to the live deck" do
    create(:email_message, email_account: account, ai_action_prompt: "Reply", received_at: 1.hour.ago)

    described_class.new.perform(user.id)

    expect(Feed::LiveDeck).to have_received(:broadcast) do |broadcast_user, items|
      expect(broadcast_user).to eq(user)
      expect(items).not_to be_empty
      expect(items).to all(be_a(FeedItem))
    end
  end

  it "hands the live deck an empty set on a no-op re-run (nothing new to insert)" do
    create(:email_message, email_account: account, ai_action_prompt: "Reply", received_at: 1.hour.ago)
    Feed::Generator.for_user(user) # already materialized

    described_class.new.perform(user.id)

    expect(Feed::LiveDeck).to have_received(:broadcast).with(user, [])
  end

  it "does nothing for a user without a workspace" do
    orphan = create(:user, workspace: workspace)
    orphan.update_columns(workspace_id: nil)

    described_class.new.perform(orphan.id)

    expect(Feed::LiveDeck).not_to have_received(:broadcast)
  end
end
