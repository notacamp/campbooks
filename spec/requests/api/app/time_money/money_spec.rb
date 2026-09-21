# frozen_string_literal: true

require "rails_helper"

RSpec.describe "API app Money surface", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:headers)   { api_app_headers(user) }

  around do |example|
    with_env("ENABLE_ACCOUNTING" => "1") do
      with_self_hosted { example.run }
    end
  end

  # ── GET /api/app/money ───────────────────────────────────────────────────────
  describe "GET /api/app/money" do
    it "returns the Money::Page read model" do
      travel_to ::Date.new(2026, 9, 20) do
        get "/api/app/money", headers: headers

        expect(response).to have_http_status(:ok)
        body = response.parsed_body["data"]
        expect(body).to include("obligations", "needs_you", "loans", "loan_suggestions", "statement_counts")
        expect(body["obligations"]).to be_an(Array)
        expect(body["needs_you"]).to be_an(Array)
        expect(body["loans"]).to be_an(Array)
      end
    end

    it "401s without a token" do
      get "/api/app/money"
      expect(response).to have_http_status(:unauthorized)
    end

    it "404s when accounting is disabled" do
      with_env("ENABLE_ACCOUNTING" => "0") do
        get "/api/app/money", headers: headers
        expect(response).to have_http_status(:not_found)
      end
    end
  end

  # ── Obligation settle / unsettle ─────────────────────────────────────────────
  describe "PATCH /api/app/money/obligations/:id/settle" do
    context "when the obligation has a document" do
      let(:document) do
        create(:document, workspace: workspace,
               document_type: :expense_invoice,
               ai_status: :completed,
               review_status: :approved)
      end

      it "marks the document as settled and returns refreshed money page" do
        travel_to ::Date.new(2026, 9, 20) do
          # Build an obligation id via Money::Ledger format ("doc:<uuid>")
          ob_id = "doc:#{document.id}"
          allow_any_instance_of(::Money::Ledger).to receive(:find).with(ob_id).and_return(
            double(document: document, missing?: true, receivable?: false)
          )
          expect(document).to receive(:mark_settled!).with(source: "manual")

          patch "/api/app/money/obligations/#{CGI.escape(ob_id)}/settle",
                headers: headers

          expect(response).to have_http_status(:ok)
        end
      end
    end
  end

  # ── GET /api/app/reconciliations ─────────────────────────────────────────────
  describe "GET /api/app/reconciliations" do
    it "returns paginated reconciliations" do
      create(:reconciliation, workspace: workspace)

      get "/api/app/reconciliations", headers: headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["data"]).to be_an(Array)
      expect(body["meta"]).to include("page", "total")
    end

    it "404s when accounting is disabled" do
      with_env("ENABLE_ACCOUNTING" => "0") do
        get "/api/app/reconciliations", headers: headers
        expect(response).to have_http_status(:not_found)
      end
    end

    it "returns empty array when another workspace has reconciliations" do
      other_workspace = create(:workspace)
      create(:reconciliation, workspace: other_workspace)

      get "/api/app/reconciliations", headers: headers

      body = response.parsed_body
      expect(body["data"]).to be_empty
    end
  end

  # ── GET /api/app/reconciliations/:id ─────────────────────────────────────────
  describe "GET /api/app/reconciliations/:id" do
    let(:reconciliation) do
      create(:reconciliation, workspace: workspace, status: :ready)
    end

    it "returns the workbench with transactions" do
      create(:bank_transaction, reconciliation: reconciliation, workspace: workspace,
             booked_on: ::Date.new(2026, 9, 1), description: "Test txn",
             amount_cents: -50_00, position: 1)

      get "/api/app/reconciliations/#{reconciliation.id}", headers: headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["data"]["reconciliation"]["id"]).to eq(reconciliation.id)
      expect(body["data"]["transactions"]).to be_an(Array)
    end

    it "404s for another workspace's reconciliation" do
      other = create(:reconciliation, workspace: create(:workspace))
      get "/api/app/reconciliations/#{other.id}", headers: headers
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── DELETE /api/app/reconciliations/:id ──────────────────────────────────────
  describe "DELETE /api/app/reconciliations/:id" do
    let(:reconciliation) { create(:reconciliation, workspace: workspace) }

    it "deletes the reconciliation" do
      delete "/api/app/reconciliations/#{reconciliation.id}", headers: headers

      expect(response).to have_http_status(:no_content)
      expect(Reconciliation.find_by(id: reconciliation.id)).to be_nil
    end

    it "404s for another workspace's reconciliation" do
      other = create(:reconciliation, workspace: create(:workspace))
      delete "/api/app/reconciliations/#{other.id}", headers: headers
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── Bank transaction workbench actions ───────────────────────────────────────
  describe "Bank transaction actions" do
    let(:reconciliation) do
      create(:reconciliation, workspace: workspace, status: :ready)
    end
    let(:transaction) do
      create(:bank_transaction, reconciliation: reconciliation, workspace: workspace,
             booked_on: ::Date.new(2026, 9, 1), description: "Coffee supplier",
             amount_cents: -120_00, status: :unmatched, position: 1)
    end

    describe "POST .../bank_transactions/:id/reset" do
      it "resets an unmatched transaction and returns the money page" do
        travel_to ::Date.new(2026, 9, 20) do
          post "/api/app/reconciliations/#{reconciliation.id}/bank_transactions/#{transaction.id}/reset",
               headers: headers

          expect(response).to have_http_status(:ok)
          body = response.parsed_body["data"]
          expect(body["transaction"]["id"]).to eq(transaction.id)
          expect(body["money"]).to be_present
        end
      end
    end

    describe "POST .../bank_transactions/:id/exclude" do
      it "excludes the transaction with a valid reason" do
        travel_to ::Date.new(2026, 9, 20) do
          post "/api/app/reconciliations/#{reconciliation.id}/bank_transactions/#{transaction.id}/exclude",
               params: { reason: "bank_fee" }, headers: headers

          expect(response).to have_http_status(:ok)
          expect(transaction.reload.status).to eq("excluded")
          expect(transaction.reload.exclusion_reason).to eq("bank_fee")
        end
      end

      it "returns 422 for an invalid exclusion reason" do
        post "/api/app/reconciliations/#{reconciliation.id}/bank_transactions/#{transaction.id}/exclude",
             params: { reason: "unicorn" }, headers: headers

        expect(response).to have_http_status(:unprocessable_entity)
      end

      it "404s for another workspace's transaction" do
        other_recon = create(:reconciliation, workspace: create(:workspace))
        other_txn   = create(:bank_transaction, reconciliation: other_recon,
                             workspace: other_recon.workspace,
                             booked_on: ::Date.new(2026, 9, 1),
                             description: "Other", amount_cents: -50_00,
                             status: :unmatched, position: 1)
        post "/api/app/reconciliations/#{other_recon.id}/bank_transactions/#{other_txn.id}/exclude",
             params: { reason: "bank_fee" }, headers: headers

        expect(response).to have_http_status(:not_found)
      end
    end

    describe "GET .../bank_transactions/:id/resolve_panel" do
      it "returns suggested matches and candidates" do
        travel_to ::Date.new(2026, 9, 20) do
          get "/api/app/reconciliations/#{reconciliation.id}/bank_transactions/#{transaction.id}/resolve_panel",
              headers: headers

          expect(response).to have_http_status(:ok)
          body = response.parsed_body["data"]
          expect(body).to include("transaction", "suggested_matches", "candidates")
        end
      end
    end
  end

  # ── Loans ─────────────────────────────────────────────────────────────────────
  describe "Loans" do
    describe "GET /api/app/money/loans" do
      it "returns active loans" do
        create(:loan, workspace: workspace, status: :active,
               lender: "Banco de Portugal",
               principal_cents: 100_000_00,
               instalment_cents: 500_00,
               first_instalment_on: ::Date.new(2026, 1, 1),
               term_months: 24)

        get "/api/app/money/loans", headers: headers

        expect(response).to have_http_status(:ok)
        loans = response.parsed_body["data"]
        expect(loans).to be_an(Array)
        expect(loans.first["lender"]).to eq("Banco de Portugal")
      end

      it "exposes the exact next upcoming instalment date" do
        travel_to ::Date.new(2026, 3, 15) do
          loan = create(:loan, workspace: workspace, status: :active,
                        first_instalment_on: ::Date.new(2026, 1, 5))
          create(:loan_instalment, loan: loan, number: 2, expected_on: ::Date.new(2026, 2, 5), status: :paid)
          create(:loan_instalment, loan: loan, number: 3, expected_on: ::Date.new(2026, 4, 5), status: :expected)

          get "/api/app/money/loans", headers: headers

          expect(response.parsed_body["data"].first["next_instalment_on"]).to eq("2026-04-05")
        end
      end
    end

    describe "POST /api/app/money/loans" do
      it "creates a loan and returns the refreshed money page" do
        travel_to ::Date.new(2026, 9, 20) do
          post "/api/app/money/loans",
               params: {
                 loan: {
                   lender: "Caixa",
                   principal: "50000",
                   instalment: "500",
                   first_instalment_on: "2026-01-01",
                   term_months: 12
                 }
               },
               headers: headers

          expect(response).to have_http_status(:created)
          body = response.parsed_body["data"]
          expect(body["loan"]["lender"]).to eq("Caixa")
          expect(body["money"]).to be_present
        end
      end
    end

    describe "DELETE /api/app/money/loans/:id" do
      it "destroys the loan and returns the refreshed money page" do
        travel_to ::Date.new(2026, 9, 20) do
          loan = create(:loan, workspace: workspace, status: :active,
                        lender: "BPI",
                        principal_cents: 50_000_00,
                        instalment_cents: 400_00,
                        first_instalment_on: ::Date.new(2026, 1, 1),
                        term_months: 12)

          delete "/api/app/money/loans/#{loan.id}", headers: headers

          expect(response).to have_http_status(:ok)
          expect(Loan.find_by(id: loan.id)).to be_nil
        end
      end

      it "404s for another workspace's loan" do
        other_loan = create(:loan, workspace: create(:workspace), status: :active,
                           lender: "Other",
                           principal_cents: 50_000_00,
                           instalment_cents: 400_00,
                           first_instalment_on: ::Date.new(2026, 1, 1),
                           term_months: 12)

        delete "/api/app/money/loans/#{other_loan.id}", headers: headers

        expect(response).to have_http_status(:not_found)
      end
    end
  end
end
