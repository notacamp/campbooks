# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Money", type: :request do
  let(:workspace) { create(:workspace, plan: "pro") }
  let(:user)      { create(:user, workspace:) }

  # A ready January 2024 statement so evidence can mark docs :missing.
  let!(:jan_stmt) do
    create(:reconciliation, :ready, :with_bank, workspace:,
           period_start: Date.new(2024, 1, 1),
           period_end:   Date.new(2024, 1, 31))
  end

  def revenue(**attrs)
    create(:document, :approved, :revenue_invoice, workspace:, currency: "EUR", **attrs)
  end

  def expense(**attrs)
    create(:document, :approved, workspace:, document_type: :expense_invoice, currency: "EUR", **attrs)
  end

  def with_flags(accounting: "1", &)
    with_env("ENABLE_ACCOUNTING" => accounting, &)
  end

  describe "gating" do
    it "404s when accounting is off" do
      with_flags(accounting: nil) { sign_in(user); get money_path }
      expect(response).to have_http_status(:not_found)
    end

    it "redirects a workspace without the accounting entitlement" do
      free      = create(:workspace, plan: "free")
      free_user = create(:user, workspace: free)
      with_flags { sign_in(free_user); get money_path }
      expect(response).to have_http_status(:redirect)
    end
  end

  describe "GET /money" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "renders the new evidence-based surface sections" do
      expense(vendor_name: "Vodafone", amount_cents: 24_800,
              document_date: Date.new(2024, 1, 10))
      get money_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Statements")
      expect(response.body).to include("Not on a statement")
    end

    it "includes the Scout read section" do
      get money_path
      expect(response.body).to include("Scout")
    end

    it "does not include forbidden language" do
      expense(vendor_name: "Vodafone", amount_cents: 24_800,
              document_date: Date.new(2024, 1, 10))
      get money_path
      expect(response.body).not_to include("You owe")
      expect(response.body).not_to match(/\d+ days? late/)
    end

    it "renders turbo-frame money_content div" do
      get money_path
      expect(response.body).to include("money_content")
    end

    it "narrows the selected statement via ?statement= param" do
      get money_path(statement: jan_stmt.id)
      expect(response).to have_http_status(:ok)
    end
  end

  describe "GET /money/statement/:id" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "renders the statement frame content" do
      get money_statement_path(jan_stmt.id)
      expect(response).to have_http_status(:ok)
    end

    it "404s for another workspace's statement" do
      other_ws   = create(:workspace)
      other_stmt = create(:reconciliation, :ready, workspace: other_ws,
                          period_start: Date.new(2024, 1, 1),
                          period_end:   Date.new(2024, 1, 31))
      get money_statement_path(other_stmt.id)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "GET /money/export.csv" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "streams a CSV with the new evidence-based headers" do
      expense(vendor_name: "Vodafone", amount_cents: 24_800,
              document_date: Date.new(2024, 1, 10))
      get money_export_path(format: :csv)

      expect(response.media_type).to eq("text/csv")
      expect(response.body).to include("Counterpart,What,Direction,Amount")
      expect(response.body).to include("Vodafone")
      expect(response.body).to include("not on a statement")
    end
  end

  describe "POST /money/obligations/:id/settle" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "marks the document as settled with source=manual by default" do
      doc = expense(vendor_name: "Vodafone", amount_cents: 24_800,
                    document_date: Date.new(2024, 1, 10))
      post money_obligation_settle_path("doc:#{doc.id}"), as: :turbo_stream

      expect(response).to have_http_status(:ok)
      expect(doc.reload).to be_settled
      expect(doc.settled_source).to eq("manual")
    end

    it "marks settled with source=elsewhere when param is set" do
      doc = expense(vendor_name: "Vodafone", amount_cents: 24_800,
                    document_date: Date.new(2024, 1, 10))
      post money_obligation_settle_path("doc:#{doc.id}"),
           params: { source: "elsewhere" }, as: :turbo_stream

      expect(doc.reload.settled_source).to eq("elsewhere")
    end

    it "404s for an obligation outside the workspace" do
      other = create(:document, :approved, document_type: :expense_invoice, amount_cents: 1_000)
      post money_obligation_settle_path("doc:#{other.id}"), as: :turbo_stream
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "DELETE /money/obligations/:id/settle (unsettle)" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "clears a manual settlement" do
      doc = expense(vendor_name: "Vodafone", amount_cents: 24_800,
                    document_date: Date.new(2024, 1, 10),
                    settled_at: Time.current, settled_source: "manual")
      delete money_obligation_settle_path("doc:#{doc.id}"), as: :turbo_stream
      expect(doc.reload).not_to be_settled
    end
  end

  describe "POST /money/obligations/:id/chase" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "opens the compose Dock for a missing receivable" do
      doc = revenue(client_name: "Brightloop", invoice_number: "0231", amount_cents: 120_000,
                    document_date: Date.new(2024, 1, 10))
      post money_obligation_chase_path("doc:#{doc.id}"), as: :turbo_stream

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("compose_dock")
      expect(response.body).to include("payment reminder")
    end

    it "404s for a missing payable (only receivables may be chased)" do
      doc = expense(vendor_name: "Vodafone", amount_cents: 24_800,
                    document_date: Date.new(2024, 1, 10))
      post money_obligation_chase_path("doc:#{doc.id}"), as: :turbo_stream
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "line actions" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    def txn(**attrs)
      create(:bank_transaction, reconciliation: jan_stmt, workspace:, **attrs)
    end

    describe "POST /money/lines/:id/confirm" do
      it "confirms a suggested match and re-renders money_content" do
        t = txn(status: :suggested)
        doc = create(:document, :approved, workspace:, document_type: :expense_invoice)
        match = t.transaction_matches.create!(document: doc, status: :suggested,
                                              matched_by: :ai, confidence: 0.9,
                                              match_reasons: {})

        post confirm_line_money_path(t.id), params: { match_id: match.id }, as: :turbo_stream
        expect(response).to have_http_status(:ok)
        expect(t.reload.status).to eq("matched")
      end
    end

    describe "POST /money/lines/:id/set_aside" do
      it "excludes a transaction with a valid reason" do
        t = txn(status: :unmatched)
        post set_aside_line_money_path(t.id), params: { reason: "bank_fee" }, as: :turbo_stream
        expect(response).to have_http_status(:ok)
        expect(t.reload.status).to eq("excluded")
      end

      it "422s for an invalid reason" do
        t = txn(status: :unmatched)
        post set_aside_line_money_path(t.id), params: { reason: "nonsense" }, as: :turbo_stream
        expect(response).to have_http_status(:unprocessable_entity)
      end

      it "404s for a transaction outside the workspace" do
        other_ws   = create(:workspace)
        other_stmt = create(:reconciliation, :ready, workspace: other_ws,
                            period_start: Date.new(2024, 1, 1),
                            period_end:   Date.new(2024, 1, 31))
        other_txn  = create(:bank_transaction, reconciliation: other_stmt, workspace: other_ws)
        post set_aside_line_money_path(other_txn.id), params: { reason: "bank_fee" }, as: :turbo_stream
        expect(response).to have_http_status(:not_found)
      end
    end

    describe "POST /money/lines/:id/reset" do
      it "resets a transaction back to unmatched" do
        t = txn(status: :excluded, exclusion_reason: "bank_fee")
        post reset_line_money_path(t.id), as: :turbo_stream
        expect(response).to have_http_status(:ok)
        expect(t.reload.status).to eq("unmatched")
      end
    end
  end

  describe "rendered actions" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "renders Resolve, Change/Confirm, the statement frame and the unbanked actions as real controls" do
      unmatched = create(:bank_transaction, reconciliation: jan_stmt, workspace:, status: :unmatched,
                         amount_cents: -20_000, booked_on: Date.new(2024, 1, 28))
      suggested = create(:bank_transaction, reconciliation: jan_stmt, workspace:, status: :suggested,
                         amount_cents: -8_432, booked_on: Date.new(2024, 1, 14))
      staples   = expense(vendor_name: "Staples", amount_cents: 8_432, invoice_number: "FT2024/0221",
                          document_date: Date.new(2024, 1, 12))
      match     = suggested.transaction_matches.create!(document: staples, status: :suggested,
                                                        matched_by: :ai, confidence: 0.78, match_reasons: {})
      missing   = expense(vendor_name: "Galp Frota", amount_cents: 8_860, document_date: Date.new(2024, 1, 22))

      get money_path
      body = response.body

      # Needs you: the hunt opens in place, the confirm is a real POST form
      expect(body).to include(resolve_panel_reconciliation_bank_transaction_path(jan_stmt, unmatched, surface: "money"))
      expect(body).to include('data-action="click->transaction-resolve#toggle"')
      expect(body).to include(confirm_line_money_path(suggested.id))
      expect(body).to include(%(name="match_id" value="#{match.id}"))
      expect(body).to include("78% likely")

      # Statements: tabs target the frame, and the frame is really there
      expect(body).to include('data-turbo-frame="money_statement"')
      expect(body).to match(/<turbo-frame[^>]*id="money_statement"/)

      # Not on a statement: settle forms for the missing receipt
      expect(body).to include(money_obligation_settle_path("doc:#{missing.id}"))
      expect(body).to include('name="source" value="elsewhere"')
      expect(body).to include("Galp Frota")
    end
  end

  describe "the hunt panel opened from Money" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "carries surface=money into its forms and its actions refresh money_content" do
      t = create(:bank_transaction, reconciliation: jan_stmt, workspace:, status: :unmatched,
                 amount_cents: -20_000, booked_on: Date.new(2024, 1, 28))

      get resolve_panel_reconciliation_bank_transaction_path(jan_stmt, t, surface: "money")
      expect(response).to have_http_status(:ok)
      expect(response.body).to include('name="surface"')

      post exclude_reconciliation_bank_transaction_path(jan_stmt, t),
           params: { reason: "bank_fee", surface: "money" }, as: :turbo_stream
      expect(response).to have_http_status(:ok)
      expect(response.body).to include('target="money_content"')
      expect(response.body).to include("surface=money")
      expect(t.reload.status).to eq("excluded")
    end

    it "leaves the workbench response alone without the surface param" do
      t = create(:bank_transaction, reconciliation: jan_stmt, workspace:, status: :unmatched)
      post exclude_reconciliation_bank_transaction_path(jan_stmt, t),
           params: { reason: "bank_fee" }, as: :turbo_stream
      expect(response.body).not_to include('target="money_content"')
    end
  end
end
