# frozen_string_literal: true

require "rails_helper"

RSpec.describe TagAccountLink, type: :model do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:tag)       { create(:tag, workspace: workspace) }

  describe "validations" do
    subject do
      build(:tag_account_link, tag: tag, email_account: account, provider_label_id: "label-1")
    end

    it { is_expected.to be_valid }
    it { is_expected.to validate_presence_of(:provider_label_id) }

    it "requires tag_id to be unique per account" do
      create(:tag_account_link, tag: tag, email_account: account, provider_label_id: "label-1")
      duplicate = build(:tag_account_link, tag: tag, email_account: account,
                        provider_label_id: "label-2")
      expect(duplicate).not_to be_valid
      expect(duplicate.errors[:tag_id]).to be_present
    end

    it "requires provider_label_id to be unique per account" do
      other_tag = create(:tag, workspace: workspace)
      create(:tag_account_link, tag: tag, email_account: account, provider_label_id: "label-1")
      duplicate = build(:tag_account_link, tag: other_tag, email_account: account,
                        provider_label_id: "label-1")
      expect(duplicate).not_to be_valid
      expect(duplicate.errors[:provider_label_id]).to be_present
    end

    it "allows the same provider_label_id on different accounts" do
      account2 = create(:email_account, workspace: workspace)
      create(:tag_account_link, tag: tag, email_account: account, provider_label_id: "label-1")
      link2 = build(:tag_account_link, tag: create(:tag, workspace: workspace),
                    email_account: account2, provider_label_id: "label-1")
      expect(link2).to be_valid
    end

    context "cross-workspace guard" do
      it "rejects a link where the account belongs to a different workspace" do
        other_ws = create(:workspace)
        other_acct = create(:email_account, workspace: other_ws)
        link = build(:tag_account_link, tag: tag, email_account: other_acct,
                     provider_label_id: "label-x")
        expect(link).not_to be_valid
        expect(link.errors[:base]).to include(a_string_matching(/same workspace/))
      end
    end
  end

  describe "associations" do
    it { is_expected.to belong_to(:tag) }
    it { is_expected.to belong_to(:email_account) }
  end
end
