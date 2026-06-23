require "rails_helper"

RSpec.describe EmailThread, type: :model do
  describe "#accessible_by?" do
    let(:workspace) { create(:workspace) }
    let(:account) { create(:email_account, workspace: workspace) }
    let(:thread) { create(:email_thread, email_account: account) }
    let(:mailbox_user) { create(:user, workspace: workspace) }
    let(:outsider) { create(:user, workspace: workspace) }

    before { create(:email_account_user, :viewer, user: mailbox_user, email_account: account) }

    it "allows a user with mailbox read access" do
      expect(thread.accessible_by?(mailbox_user)).to be true
    end

    it "denies a user with neither mailbox access nor a follow" do
      expect(thread.accessible_by?(outsider)).to be false
    end

    it "allows a teammate pulled in by a mention (follows the thread, no mailbox access)" do
      agent_thread = create(:agent_thread, workspace: workspace, user: mailbox_user, contextable: thread)
      create(:thread_follow, user: outsider, agent_thread: agent_thread)
      expect(thread.accessible_by?(outsider)).to be true
    end
  end
end
