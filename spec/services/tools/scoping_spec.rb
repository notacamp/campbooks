require "rails_helper"

# Proves the Scout tool loop is gated to the acting user's permitted data.
# Before this, the tools queried EmailMessage.all / Document.all across every
# workspace — a cross-tenant leak when Scout ran in a session-less background job.
RSpec.describe "Scout tool permission scoping", type: :service do
  let(:workspace) { create(:workspace) }
  let(:other_workspace) { create(:workspace) }
  let(:alice) { create(:user, workspace: workspace) }

  let(:alice_account) { create(:email_account, workspace: workspace) }
  let(:teammate_account) { create(:email_account, workspace: workspace) }      # same workspace, not shared with alice
  let(:foreign_account) { create(:email_account, workspace: other_workspace) } # different workspace entirely

  let!(:alice_message)    { create(:email_message, email_account: alice_account, subject: "Alice invoice") }
  let!(:teammate_message) { create(:email_message, email_account: teammate_account, subject: "Teammate secret") }
  let!(:foreign_message)  { create(:email_message, email_account: foreign_account, subject: "Other workspace") }

  before do
    create(:email_account_user, :viewer, user: alice, email_account: alice_account)
    # Simulate the background job establishing the acting identity.
    Current.acting_user = alice
    Current.workspace = workspace
  end

  after { Current.reset }

  describe Tools::QueryEmails do
    it "returns only messages on the acting user's readable accounts" do
      result = described_class.call({})
      subjects = result[:messages].map { |m| m[:subject] }

      expect(subjects).to eq([ "Alice invoice" ])
      expect(result[:count]).to eq(1)
    end

    it "fails closed when no user is established" do
      Current.acting_user = nil
      expect(described_class.call({})[:count]).to eq(0)
    end
  end

  describe Tools::SystemStats do
    it "counts only the acting user's accessible emails" do
      expect(described_class.call.dig(:emails, :total)).to eq(1)
    end
  end

  describe Tools::BulkArchive do
    it "ignores messages on accounts the user cannot read" do
      result = described_class.call("email_ids" => [ alice_message.id, teammate_message.id, foreign_message.id ])
      # Only alice's own message is in scope (and there is no real provider to hit).
      expect(result[:archived_count]).to eq(1)
    end
  end

  describe Tools::Executor do
    it "blocks an email-scoped tool on an account the user cannot read" do
      result = described_class.call(tool: "archive", email_message: teammate_message, args: {})

      expect(result[:success]).to be(false)
      expect(result[:message]).to match(/access/i)
    end

    it "blocks forwarding from an account the user can read but cannot send from" do
      result = described_class.call(tool: "forward_email", email_message: alice_message, args: { to_address: "x@example.com" })

      expect(result[:success]).to be(false)
      expect(result[:message]).to match(/send/i)
    end

    it "fails closed when no user is established" do
      Current.acting_user = nil
      result = described_class.call(tool: "archive", email_message: alice_message, args: {})

      expect(result[:success]).to be(false)
    end
  end
end
