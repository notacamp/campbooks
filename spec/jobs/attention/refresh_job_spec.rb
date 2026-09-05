# frozen_string_literal: true

require "rails_helper"

RSpec.describe Attention::RefreshJob do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }

  describe ".enqueue_for" do
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
      Rails.cache.delete("attention_refresh_pending_#{user.id}")

      expect {
        described_class.enqueue_for(user.id)
      }.to have_enqueued_job(described_class).with(user.id)
    end

    it "no-ops on a blank user_id" do
      expect { described_class.enqueue_for(nil) }.not_to have_enqueued_job(described_class)
    end
  end

  describe "#perform" do
    it "calls Attention::Refresh.call for the user" do
      allow(Attention::Refresh).to receive(:call)
      described_class.new.perform(user.id)
      expect(Attention::Refresh).to have_received(:call).with(user)
    end

    it "is a no-op for a user without a workspace" do
      orphan = create(:user, workspace: workspace)
      orphan.update_columns(workspace_id: nil)
      allow(Attention::Refresh).to receive(:call)

      described_class.new.perform(orphan.id)

      expect(Attention::Refresh).not_to have_received(:call)
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
