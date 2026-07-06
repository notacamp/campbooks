# frozen_string_literal: true

require "rails_helper"

RSpec.describe LabelImportDecision, type: :model do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:tag)       { create(:tag, workspace: workspace) }

  describe "validations" do
    subject do
      build(:label_import_decision,
            email_account: account,
            provider_label_id: "label-1",
            provider_label_name: "Newsletter")
    end

    it { is_expected.to be_valid }
    it { is_expected.to validate_presence_of(:provider_label_id) }
    it { is_expected.to validate_presence_of(:provider_label_name) }

    it "requires provider_label_id to be unique per account" do
      create(:label_import_decision, email_account: account, provider_label_id: "label-1",
             provider_label_name: "A")
      dup = build(:label_import_decision, email_account: account, provider_label_id: "label-1",
                  provider_label_name: "B")
      expect(dup).not_to be_valid
    end
  end

  describe "scopes" do
    let!(:pending_row)  { create(:label_import_decision, email_account: account,
                                 provider_label_id: "scope-label-1", decision: :pending) }
    let!(:kept_row)     { create(:label_import_decision, email_account: account,
                                 provider_label_id: "scope-label-2", decision: :kept) }

    it ".pending_review returns only pending rows" do
      expect(described_class.pending_review).to include(pending_row)
      expect(described_class.pending_review).not_to include(kept_row)
    end

    it ".resolved returns non-pending rows" do
      expect(described_class.resolved).to include(kept_row)
      expect(described_class.resolved).not_to include(pending_row)
    end

    it ".for_workspace scopes by workspace" do
      other_ws   = create(:workspace)
      other_acct = create(:email_account, workspace: other_ws)
      other_dec  = create(:label_import_decision, email_account: other_acct)
      expect(described_class.for_workspace(workspace)).not_to include(other_dec)
      expect(described_class.for_workspace(workspace)).to include(pending_row)
    end
  end

  describe "#resolve!" do
    let(:user) { create(:user, workspace: workspace) }
    subject(:decision) do
      create(:label_import_decision, email_account: account, decision: :pending)
    end

    it "marks the row as mapped and stamps the reviewer" do
      decision.resolve!(decision: :mapped, tag: tag, reviewed_by: user)
      expect(decision.reload.decision).to eq("mapped")
      expect(decision.tag).to eq(tag)
      expect(decision.reviewed_by).to eq(user)
      expect(decision.reviewed_at).to be_within(2.seconds).of(Time.current)
    end

    it "is a no-op when already resolved" do
      decision.update!(decision: :kept)
      expect { decision.resolve!(decision: :mapped) }
        .not_to change { decision.reload.decision }
    end
  end
end
