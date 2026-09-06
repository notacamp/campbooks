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

  describe "the months" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    let!(:dec_stmt) do
      create(:reconciliation, :ready, :with_bank, workspace:,
             period_start: Date.new(2023, 12, 1), period_end: Date.new(2023, 12, 31))
    end

    it "leads with the most recent month to reconcile and keeps every unreconciled month in view" do
      travel_to(Date.new(2024, 4, 15)) { get money_path }
      body = response.body

      expect(body).to include("March · no statement yet")
      expect(body).to include("February · no statement")
      expect(body.index("March · no statement yet")).to be < body.index("February · no statement")
      expect(body.index("February · no statement")).to be < body.index(%(statement=#{jan_stmt.id}))
      expect(body).to match(/March(&#39;|')s statement isn(&#39;|')t in yet/)
      expect(body).to include("Add a statement")
    end

    it "carries the year on months from another year" do
      travel_to(Date.new(2025, 9, 6)) { get money_path }
      expect(response.body).to include("January 2024")
      expect(response.body).to include("August · no statement yet")
    end

    it "marks the clicked month active, in the page and in the frame" do
      get money_path(statement: dec_stmt.id)
      expect(response.body).to match(/statement=#{dec_stmt.id}"[^>]*aria-selected="true"/)
      expect(response.body).to match(/statement=#{jan_stmt.id}"[^>]*aria-selected="false"/)

      get money_statement_path(dec_stmt.id)
      expect(response.body).to match(/statement=#{dec_stmt.id}"[^>]*aria-selected="true"/)
      expect(response.body).to match(/statement=#{jan_stmt.id}"[^>]*aria-selected="false"/)
    end

    it "only counts an invoice as missing when its month is reconciled" do
      create(:reconciliation, :ready, :with_bank, workspace:,
             period_start: Date.new(2023, 9, 1), period_end: Date.new(2023, 9, 30))
      expense(vendor_name: "Galp Frota", amount_cents: 8_860, document_date: Date.new(2023, 10, 12)) # the gap
      expense(vendor_name: "Vodafone", amount_cents: 24_800, document_date: Date.new(2023, 12, 10))

      travel_to(Date.new(2024, 2, 15)) { get money_path }
      body = response.body
      expect(body).to include("Vodafone")
      expect(body).not_to include("Galp Frota")
      expect(body).to match(/One invoice from a reconciled month isn(&#39;|')t on a statement/)
    end
  end

  describe "statements Scout already holds" do
    around { |ex| with_flags { ex.run } }
    before { sign_in(user) }

    it "offers to reconcile them, all at once or one by one" do
      create(:document, :bank_statement, :approved, workspace:, bank_name: "Millennium BCP")
      get money_path
      body = response.body

      expect(body).to include(reconcile_statements_money_path)
      expect(body).to include("Pick which")
      expect(body).to include(new_reconciliation_path)
      expect(body).to match(/A bank statement from your email isn(&#39;|')t reconciled yet/)
    end

    it "starts one reconciliation per held statement in the background" do
      create(:document, :bank_statement, :approved, workspace:)
      create(:document, :bank_statement, :approved, workspace:)
      done = create(:document, :bank_statement, :approved, workspace:)
      create(:reconciliation, workspace:, statement_document: done, created_by: user)

      expect { post reconcile_statements_money_path, as: :turbo_stream }
        .to have_enqueued_job(Reconciliations::AutoStartJob).exactly(2).times
      expect(response).to have_http_status(:ok)
      expect(response.body).to include("Reconciling 2 statements")
      expect(response.body).to include('target="money_content"')
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

  describe "loan card rendering" do
    around { |ex| with_flags { ex.run } }
    before do
      sign_in(user)
      allow_any_instance_of(Loans::Spotter).to receive(:call).and_return([])
    end

    it "renders a LoanCard when an active loan exists" do
      loan = create(:loan, :with_schedule, workspace:, created_by: user,
                           lender: "Millennium BCP",
                           first_instalment_on: Date.new(2024, 1, 5),
                           term_months: 60)

      get money_path
      expect(response).to have_http_status(:ok)
      expect(response.body).to include(loan.lender)
    end

    it "renders a LoanSuggestionRow when Spotter finds a suggestion" do
      suggestion = Loans::Spotter::Suggestion.new(
        lender_guess: "Test Bank",
        source_counterparty: "TEST BANK",
        instalment_cents: 78_000,
        currency: "EUR",
        first_seen_on: 1.year.ago.to_date,
        last_seen_on: 1.month.ago.to_date,
        count: 12,
        day_of_month: 5,
        previous_instalment_cents: nil,
        sample_transaction_ids: [],
        key: "test bank|78000"
      )
      allow_any_instance_of(Loans::Spotter).to receive(:call).and_return([ suggestion ])

      get money_path
      expect(response).to have_http_status(:ok)
      # The suggestion row renders the lender guess
      expect(response.body).to include("Test Bank")
    end
  end
end
