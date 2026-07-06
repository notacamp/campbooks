# frozen_string_literal: true

require "rails_helper"

RSpec.describe Tags::BackfillAccountLinksJob, type: :job do
  let(:workspace) { create(:workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  describe "#perform" do
    context "with existing external tags" do
      let!(:ext_tag) do
        create(:tag, :external, workspace: workspace, email_account: account,
               external_label_id: "label-abc", name: "Invoices")
      end

      it "creates a TagAccountLink for each external tag" do
        expect { described_class.perform_now }
          .to change(TagAccountLink, :count).by(1)

        link = TagAccountLink.last
        expect(link.tag).to eq(ext_tag)
        expect(link.email_account).to eq(account)
        expect(link.provider_label_id).to eq("label-abc")
      end

      it "creates a 'kept' LabelImportDecision for each external tag" do
        expect { described_class.perform_now }
          .to change(LabelImportDecision, :count).by(1)

        dec = LabelImportDecision.last
        expect(dec.email_account).to eq(account)
        expect(dec.provider_label_id).to eq("label-abc")
        expect(dec.decision).to eq("kept")
      end

      it "is idempotent — running twice does not duplicate rows" do
        described_class.perform_now
        expect { described_class.perform_now }
          .not_to change(TagAccountLink, :count)
        expect { described_class.perform_now }
          .not_to change(LabelImportDecision, :count)
      end
    end

    context "with local tags (no external_label_id)" do
      let!(:local_tag) { create(:tag, workspace: workspace, source: :local) }

      it "skips local tags" do
        expect { described_class.perform_now }
          .not_to change(TagAccountLink, :count)
      end
    end

    context "when an individual tag fails" do
      let!(:ext_tag) do
        create(:tag, :external, workspace: workspace, email_account: account,
               external_label_id: "label-xyz", name: "Receipts")
      end

      it "logs and continues rather than aborting the batch" do
        allow(TagAccountLink).to receive(:find_or_create_by!).and_raise("db error")
        expect(Rails.logger).to receive(:error).at_least(:once)
        expect { described_class.perform_now }.not_to raise_error
      end
    end
  end
end
