# frozen_string_literal: true

require "rails_helper"

RSpec.describe "InboxSettings::LabelReviewsController", type: :request do
  before { sign_in_as(user) }
  after  { Rails.cache.clear }

  let(:workspace) { create(:workspace) }
  let(:user)      { workspace.users.create!(name: "Reviewer", email_address: "reviewer-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:account)   { create(:email_account, workspace: workspace, provider: :google) }

  def create_pending(label_id: "label-1", name: "Newsletter")
    create(:label_import_decision,
           email_account: account,
           provider_label_id: label_id,
           provider_label_name: name,
           decision: :pending)
  end

  describe "GET /inbox_settings/label_reviews" do
    it "renders the review panel" do
      create_pending
      get inbox_settings_label_reviews_path
      expect(response).to have_http_status(:ok)
    end

    it "shows only pending decisions for the current workspace" do
      other_ws   = create(:workspace)
      other_acct = create(:email_account, workspace: other_ws)
      create(:label_import_decision, email_account: other_acct, provider_label_id: "x",
             decision: :pending)
      pending_row = create_pending
      get inbox_settings_label_reviews_path
      expect(response.body).to include(pending_row.provider_label_name)
    end
  end

  describe "PATCH /inbox_settings/label_reviews/bulk_decide" do
    let!(:pending_row) { create_pending(label_id: "label-1", name: "Newsletter") }
    let(:existing_tag) { create(:tag, workspace: workspace, name: "My Newsletter Tag") }

    context "decision: kept" do
      it "marks the decision as kept" do
        patch bulk_decide_inbox_settings_label_reviews_path,
              params: { decisions: { pending_row.id => { decision: "kept" } } },
              headers: { "Accept" => "text/vnd.turbo-stream.html" }

        expect(pending_row.reload.decision).to eq("kept")
      end
    end

    context "decision: mapped" do
      it "marks the decision as mapped and links the tag" do
        ext_tag = account.external_tags.create!(
          workspace: workspace, name: "Newsletter", color: "#aaa",
          source: :external, external_label_id: "label-1"
        )

        patch bulk_decide_inbox_settings_label_reviews_path,
              params: { decisions: { pending_row.id => { decision: "mapped", tag_id: existing_tag.id } } },
              headers: { "Accept" => "text/vnd.turbo-stream.html" }

        expect(pending_row.reload.decision).to eq("mapped")
        expect(pending_row.tag).to eq(existing_tag)
        expect(TagAccountLink.where(tag: existing_tag, email_account: account)).to exist
      end

      it "ignores a mapped decision with no tag_id" do
        patch bulk_decide_inbox_settings_label_reviews_path,
              params: { decisions: { pending_row.id => { decision: "mapped", tag_id: "" } } },
              headers: { "Accept" => "text/vnd.turbo-stream.html" }

        expect(pending_row.reload.decision).to eq("pending")
      end
    end

    context "decision: ignored" do
      it "marks the decision as ignored without creating a tag or link" do
        patch bulk_decide_inbox_settings_label_reviews_path,
              params: { decisions: { pending_row.id => { decision: "ignored" } } },
              headers: { "Accept" => "text/vnd.turbo-stream.html" }

        expect(pending_row.reload.decision).to eq("ignored")
        expect(TagAccountLink.count).to eq(0)
      end
    end

    it "cannot resolve decisions from a different workspace" do
      other_ws   = create(:workspace)
      other_acct = create(:email_account, workspace: other_ws)
      other_dec  = create(:label_import_decision, email_account: other_acct,
                           provider_label_id: "x", decision: :pending)

      patch bulk_decide_inbox_settings_label_reviews_path,
            params: { decisions: { other_dec.id => { decision: "ignored" } } },
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      # The decision from another workspace must remain untouched.
      expect(other_dec.reload.decision).to eq("pending")
    end

    it "skips already-resolved rows" do
      pending_row.update!(decision: :kept)

      patch bulk_decide_inbox_settings_label_reviews_path,
            params: { decisions: { pending_row.id => { decision: "ignored" } } },
            headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(pending_row.reload.decision).to eq("kept")
    end
  end
end
