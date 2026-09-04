# frozen_string_literal: true

require "rails_helper"

RSpec.describe People::StandingsRefreshJob do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    allow(Features).to receive(:bold_layout?).and_return(true)
  end

  describe ".enqueue_for" do
    # Use a real MemoryStore so the unless_exist debounce gate works correctly.
    around do |example|
      original_store = Rails.cache
      Rails.cache = ActiveSupport::Cache::MemoryStore.new
      example.run
    ensure
      Rails.cache = original_store
    end

    it "enqueues the job within the debounce window" do
      expect {
        described_class.enqueue_for(user.id)
      }.to have_enqueued_job(described_class).with(user.id)
    end

    it "debounces within the window (second call is a no-op)" do
      described_class.enqueue_for(user.id)
      expect {
        described_class.enqueue_for(user.id)
      }.not_to have_enqueued_job(described_class)
    end

    it "enqueues again after the debounce window has expired" do
      described_class.enqueue_for(user.id, debounce: 1.second)
      Rails.cache.delete("people_standings_pending_#{user.id}")  # simulate expiry

      expect {
        described_class.enqueue_for(user.id)
      }.to have_enqueued_job(described_class).with(user.id)
    end

    it "no-ops on a blank user_id" do
      expect { described_class.enqueue_for(nil) }.not_to have_enqueued_job(described_class)
    end
  end

  describe "#perform" do
    it "calls People::Standings.refresh! for the user" do
      allow(People::Standings).to receive(:refresh!)
      described_class.new.perform(user.id)
      expect(People::Standings).to have_received(:refresh!).with(user)
    end

    it "is a no-op for a user without a workspace" do
      orphan = create(:user, workspace: workspace)
      orphan.update_columns(workspace_id: nil)
      allow(People::Standings).to receive(:refresh!)

      described_class.new.perform(orphan.id)

      expect(People::Standings).not_to have_received(:refresh!)
    end
  end

  describe ".enqueue_for_workspace" do
    it "enqueues for every user in the workspace" do
      other_user = create(:user, workspace: workspace)
      expect {
        described_class.enqueue_for_workspace(workspace)
      }.to have_enqueued_job(described_class).with(user.id)
        .and have_enqueued_job(described_class).with(other_user.id)
    end
  end
end
