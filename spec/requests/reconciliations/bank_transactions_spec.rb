# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Reconciliations::BankTransactions", type: :request do
  let(:workspace)      { create(:workspace, plan: "pro") }
  let(:user)           { create(:user, workspace:) }
  let(:reconciliation) { create(:reconciliation, workspace:, created_by: user) }
  let(:document) do
    create(:document,
           workspace:,
           document_type: :expense_invoice,
           amount_cents:  5000,
           currency:      "EUR",
           document_date: Date.new(2024, 1, 10))
  end
  let!(:txn) do
    create(:bank_transaction,
           reconciliation:,
           workspace:,
           booked_on:    Date.new(2024, 1, 15),
           description:  "Payment vendor",
           amount_cents: -5000,
           currency:     "EUR")
  end

  around do |example|
    with_env("ENABLE_ACCOUNTING" => "1") { example.run }
  end

  # ── Authentication guard ─────────────────────────────────────────────────────

  describe "unauthenticated requests" do
    it "redirects confirm" do
      post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/confirm"
      expect(response).to redirect_to(/session/)
    end
  end

  # ── POST confirm ─────────────────────────────────────────────────────────────

  describe "POST /reconciliations/:reconciliation_id/bank_transactions/:id/confirm" do
    let(:match) do
      create(:transaction_match,
             bank_transaction: txn,
             document:,
             status:     :suggested,
             matched_by: :heuristic,
             confidence: 0.9)
    end

    before { sign_in(user) }

    it "confirms the match and returns turbo_stream" do
      post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/confirm",
           params:  { match_id: match.id },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(match.reload.status).to eq("confirmed")
      expect(txn.reload.status).to eq("matched")
    end

    it "returns 404 for a match belonging to a different workspace" do
      other_ws = create(:workspace)
      other_txn = create(:bank_transaction,
                          reconciliation: create(:reconciliation, workspace: other_ws),
                          workspace:      other_ws)
      other_match = create(:transaction_match,
                            bank_transaction: other_txn,
                            document:         create(:document, workspace: other_ws))

      post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/confirm",
           params: { match_id: other_match.id }
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── POST reject ──────────────────────────────────────────────────────────────

  describe "POST /reconciliations/:reconciliation_id/bank_transactions/:id/reject" do
    let!(:match) do
      create(:transaction_match,
             bank_transaction: txn,
             document:,
             status:     :suggested,
             confidence: 0.8)
      txn.update!(status: :suggested)
      txn.transaction_matches.last
    end

    before { sign_in(user) }

    it "marks the match rejected and returns turbo_stream" do
      post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/reject",
           params:  { match_id: match.id },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(match.reload.status).to eq("rejected")
    end

    it "reverts txn to :unmatched when no suggested matches remain" do
      post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/reject",
           params: { match_id: match.id }
      expect(txn.reload.status).to eq("unmatched")
    end
  end

  # ── POST exclude ─────────────────────────────────────────────────────────────

  describe "POST /reconciliations/:reconciliation_id/bank_transactions/:id/exclude" do
    before { sign_in(user) }

    it "sets :excluded status and exclusion_reason" do
      post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/exclude",
           params:  { reason: "bank_fee" },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(txn.reload.status).to eq("excluded")
      expect(txn.reload.exclusion_reason).to eq("bank_fee")
    end

    it "rejects invalid reason with 422" do
      post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/exclude",
           params:  { reason: "invalid_reason" },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:unprocessable_entity)
      expect(txn.reload.status).to eq("unmatched")
    end
  end

  # ── POST reset ───────────────────────────────────────────────────────────────

  describe "POST /reconciliations/:reconciliation_id/bank_transactions/:id/reset" do
    before do
      txn.update!(status: :excluded, exclusion_reason: "bank_fee")
      sign_in(user)
    end

    it "resets to :unmatched and clears exclusion_reason" do
      post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/reset",
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(txn.reload.status).to eq("unmatched")
      expect(txn.reload.exclusion_reason).to be_nil
    end
  end

  # ── POST manual_match ────────────────────────────────────────────────────────

  describe "POST /reconciliations/:reconciliation_id/bank_transactions/:id/manual_match" do
    before { sign_in(user) }

    it "creates a confirmed match and sets :matched status" do
      post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/manual_match",
           params:  { document_id: document.id },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(txn.reload.status).to eq("matched")
      match = txn.transaction_matches.find_by(document:)
      expect(match).to be_present
      expect(match.status).to eq("confirmed")
      expect(match.matched_by).to eq("manual")
      expect(match.confidence).to eq(1.0)
    end

    it "returns 404 for a document not in the workspace" do
      other_doc = create(:document, workspace: create(:workspace))
      post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/manual_match",
           params: { document_id: other_doc.id }
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── GET resolve_panel ────────────────────────────────────────────────────────

  describe "GET /reconciliations/:reconciliation_id/bank_transactions/:id/resolve_panel" do
    before { sign_in(user) }

    it "returns 200 with html response" do
      get "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/resolve_panel"
      expect(response).to have_http_status(:ok)
    end

    it "filters candidates by search query q" do
      doc_matching = create(:document,
                             workspace:,
                             document_type: :expense_invoice,
                             amount_cents:  5000,
                             vendor_name:   "Target Vendor")
      create(:document,
             workspace:,
             document_type: :expense_invoice,
             amount_cents:  3000,
             vendor_name:   "Other Corp")

      get "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/resolve_panel",
          params: { q: "Target", format: :turbo_stream }

      expect(response).to have_http_status(:ok)
      expect(response.body).to include(doc_matching.id.to_s)
    end
  end

  # ── POST request_invoice ─────────────────────────────────────────────────────

  describe "POST /reconciliations/:reconciliation_id/bank_transactions/:id/request_invoice" do
    before { sign_in(user) }

    context "with an unmatched transaction" do
      it "marks the transaction as requested and opens the composer" do
        expect(txn.status).to eq("unmatched")

        post "/reconciliations/#{reconciliation.id}/bank_transactions/#{txn.id}/request_invoice",
             headers: { "Accept" => "text/vnd.turbo-stream.html" }

        expect(response).to have_http_status(:ok)
        txn.reload
        expect(txn.status).to eq("requested")
        expect(txn.requested_at).to be_present
        expect(txn.requested_by_id).to eq(user.id)
        expect(response.body).to include("compose_dock")
      end
    end

    context "with a matched transaction (no NIF flag)" do
      let!(:matched_txn) do
        create(:bank_transaction,
               reconciliation:,
               workspace:,
               booked_on:    Date.new(2024, 1, 15),
               description:  "Already matched",
               amount_cents: -5000,
               currency:     "EUR",
               status:       :matched)
      end

      it "returns an info flash without changing the transaction" do
        post "/reconciliations/#{reconciliation.id}/bank_transactions/#{matched_txn.id}/request_invoice",
             headers: { "Accept" => "text/vnd.turbo-stream.html" }

        expect(response).to have_http_status(:ok)
        expect(matched_txn.reload.status).to eq("matched")
        expect(matched_txn.reload.requested_at).to be_nil
      end
    end

    context "with a cross-workspace transaction" do
      it "returns 404" do
        other_recon = create(:reconciliation, workspace: create(:workspace))
        other_txn = create(:bank_transaction,
                           reconciliation: other_recon,
                           workspace: other_recon.workspace,
                           booked_on: Date.today,
                           description: "Cross-ws",
                           amount_cents: -100,
                           currency: "EUR")

        post "/reconciliations/#{other_recon.id}/bank_transactions/#{other_txn.id}/request_invoice",
             headers: { "Accept" => "text/vnd.turbo-stream.html" }

        expect(response).to have_http_status(:not_found)
      end
    end
  end

  # ── Entitlement guard ────────────────────────────────────────────────────────

  describe "entitlement check on mutation actions" do
    let(:free_user) { create(:user, workspace: create(:workspace, plan: "free")) }
    let(:free_recon) { create(:reconciliation, workspace: free_user.workspace, created_by: free_user) }
    let!(:free_txn) do
      create(:bank_transaction,
             reconciliation: free_recon,
             workspace:      free_user.workspace,
             booked_on:      Date.today,
             description:    "Test",
             amount_cents:   -1000,
             currency:       "EUR")
    end

    before { sign_in(free_user) }

    it "returns 402 for confirm on free plan" do
      match = create(:transaction_match,
                     bank_transaction: free_txn,
                     document: create(:document, workspace: free_user.workspace))
      post "/reconciliations/#{free_recon.id}/bank_transactions/#{free_txn.id}/confirm",
           params:  { match_id: match.id },
           headers: { "Accept" => "application/json" }
      expect(response).to have_http_status(:payment_required)
    end
  end
end
