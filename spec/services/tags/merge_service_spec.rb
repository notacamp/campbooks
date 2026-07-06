# frozen_string_literal: true

require "rails_helper"

RSpec.describe Tags::MergeService, type: :service do
  let(:workspace) { create(:workspace) }
  let(:source)    { create(:tag, workspace: workspace, name: "Source Tag", color: "#ff0000") }
  let(:target)    { create(:tag, workspace: workspace, name: "Target Tag", color: "#00ff00") }

  subject(:service) { described_class.new(source: source, target: target) }

  describe "#merge!" do
    it "destroys the source tag" do
      service.merge!
      expect(Tag.exists?(source.id)).to be false
    end

    it "returns the surviving target tag" do
      result = service.merge!
      expect(result).to eq(target)
    end

    context "message tag migration" do
      let(:account) { create(:email_account, workspace: workspace) }
      let(:msg1) do
        create(:email_message, email_account: account).tap do |m|
          EmailMessageTag.create!(email_message: m, tag: source)
        end
      end
      let(:msg2) do
        create(:email_message, email_account: account).tap do |m|
          EmailMessageTag.create!(email_message: m, tag: source)
          EmailMessageTag.create!(email_message: m, tag: target)
        end
      end

      before { msg1; msg2 }

      it "moves source message tags to the target" do
        service.merge!
        expect(target.email_messages).to include(msg1)
      end

      it "deduplicates messages already tagged with the target" do
        service.merge!
        count = EmailMessageTag.where(email_message: msg2, tag: target).count
        expect(count).to eq(1)
      end

      it "removes all source EmailMessageTag rows" do
        service.merge!
        expect(EmailMessageTag.where(tag: source)).to be_empty
      end
    end

    context "account link migration" do
      let(:account) { create(:email_account, workspace: workspace) }

      it "moves TagAccountLinks from source to target" do
        link = create(:tag_account_link, tag: source, email_account: account,
                      provider_label_id: "lab-1")
        service.merge!
        expect(TagAccountLink.exists?(link.id)).to be false
        expect(TagAccountLink.where(tag: target, provider_label_id: "lab-1")).to exist
      end

      it "deduplicates links the target already has for the same account" do
        create(:tag_account_link, tag: source, email_account: account,
               provider_label_id: "lab-1")
        create(:tag_account_link, tag: target, email_account: account,
               provider_label_id: "lab-2")
        expect { service.merge! }.not_to raise_error
        expect(TagAccountLink.where(tag: target).count).to eq(1)
      end
    end

    context "default_bucket adoption" do
      it "copies source default_bucket to target when target has none" do
        source.update!(default_bucket: "promotions")
        service.merge!
        expect(target.reload.default_bucket).to eq("promotions")
      end

      it "keeps target default_bucket when both have one" do
        source.update!(default_bucket: "social")
        target.update!(default_bucket: "updates")
        service.merge!
        expect(target.reload.default_bucket).to eq("updates")
      end

      it "does NOT copy source group_name when target is ungrouped" do
        source.update!(default_bucket: "promotions", group_name: "Noise")
        service.merge!
        expect(target.reload.group_name).to be_nil
      end

      it "does NOT change target group_name when target is already grouped" do
        source.update!(default_bucket: "promotions", group_name: "Noise")
        target.update!(group_name: "Important")
        service.merge!
        expect(target.reload.group_name).to eq("Important")
      end
    end

    context "import decision re-pointing" do
      let(:account) { create(:email_account, workspace: workspace) }

      it "re-points LabelImportDecisions from source to target" do
        dec = create(:label_import_decision, email_account: account, tag: source,
                     decision: :kept)
        service.merge!
        expect(dec.reload.tag).to eq(target)
      end
    end

    context "guard failures" do
      it "raises when merging a tag into itself" do
        expect { described_class.new(source: source, target: source).merge! }
          .to raise_error(Tags::MergeService::MergeError, /itself/)
      end

      it "raises when merging across workspaces" do
        other_ws  = create(:workspace)
        other_tag = create(:tag, workspace: other_ws)
        expect { described_class.new(source: source, target: other_tag).merge! }
          .to raise_error(Tags::MergeService::MergeError, /workspace/)
      end

      it "rolls back on failure — source survives" do
        # Create a message tag so insert_all is actually called.
        acct = create(:email_account, workspace: workspace)
        msg  = create(:email_message, email_account: acct)
        EmailMessageTag.create!(email_message: msg, tag: source)

        allow(EmailMessageTag).to receive(:insert_all).and_raise("boom")
        expect { service.merge! }.to raise_error("boom")
        expect(Tag.exists?(source.id)).to be true
      end
    end
  end
end
