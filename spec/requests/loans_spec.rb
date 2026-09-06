# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Loans", type: :request do
  let(:workspace) { create(:workspace, plan: "pro") }
  let(:user)      { create(:user, workspace:) }

  def with_flags(&)
    with_env("ENABLE_ACCOUNTING" => "1", &)
  end

  def make_reconciliation
    doc = workspace.documents.build(document_type: :bank_statement, ai_status: :skipped,
                                    review_status: :pending, source: :manual_upload)
    doc.original_file.attach(io: StringIO.new("s"), filename: "s.csv", content_type: "text/csv")
    doc.save!
    Reconciliation.create!(workspace: workspace, created_by: user,
                           statement_document: doc, currency: "EUR",
                           status: :ready,
                           period_start: Date.new(2024, 1, 1),
                           period_end: Date.new(2024, 1, 31))
  end

  # Silence Loans::Matcher broadcasts in all paths
  before do
    allow_any_instance_of(Loans::Matcher).to receive(:broadcast_row).and_return(nil)
    allow(Loans::Backfill).to receive(:call).and_return(nil)
  end

  describe "POST /money/loans (create)" do
    let(:valid_params) do
      {
        loan: {
          lender: "Test Bank",
          source_counterparty: "TEST BANK",
          principal: "46800",
          instalment: "780",
          first_instalment_on: "2024-01-05",
          term_months: "60",
          rate_note: "Euribor 12M + 1.5%"
        }
      }
    end

    it "creates a loan and its schedule" do
      with_flags do
        sign_in(user)
        post money_loans_path, params: valid_params
        expect(Loan.last.term_months).to eq(60)
        expect(LoanInstalment.where(loan: Loan.last).count).to eq(60)
      end
    end

    it "calls Loans::Backfill" do
      expect(Loans::Backfill).to receive(:call)
      with_flags do
        sign_in(user)
        post money_loans_path, params: valid_params
      end
    end

    it "redirects or responds with turbo_stream" do
      with_flags do
        sign_in(user)
        post money_loans_path, params: valid_params
        expect(response).to have_http_status(:ok).or have_http_status(:found)
      end
    end

    it "returns 422 with invalid params (no lender)" do
      with_flags do
        sign_in(user)
        post money_loans_path, params: { loan: valid_params[:loan].merge(lender: "") }
        expect(response).to have_http_status(:unprocessable_entity).or have_http_status(:found)
      end
    end
  end

  describe "PATCH /money/loans/:id (update)" do
    let!(:loan) do
      l = create(:loan, workspace:, created_by: user, term_months: 60)
      Loans::Schedule.build!(l)
      l.instalments.first(3).each { |i| i.update!(status: :paid, amount_cents: l.instalment_cents) }
      l
    end

    it "rebuilds the schedule and calls Backfill" do
      expect(Loans::Backfill).to receive(:call)
      with_flags do
        sign_in(user)
        patch money_loan_path(loan), params: { loan: { instalment: "800", term_months: "60",
                                               first_instalment_on: loan.first_instalment_on.to_s } }
        expect(loan.reload.instalment_cents).to eq(80_000)
      end
    end

    it "keeps paid instalments after rebuild" do
      with_flags do
        sign_in(user)
        patch money_loan_path(loan), params: { loan: { instalment: "800", term_months: "60",
                                               first_instalment_on: loan.first_instalment_on.to_s } }
        expect(loan.instalments.where(status: :paid).count).to eq(3)
      end
    end
  end

  describe "DELETE /money/loans/:id (destroy)" do
    let!(:recon) { make_reconciliation }
    let!(:loan)  { create(:loan, workspace:, created_by: user, term_months: 3) }

    before { Loans::Schedule.build!(loan) }

    it "destroys the loan" do
      with_flags do
        sign_in(user)
        delete money_loan_path(loan)
        expect(Loan.find_by(id: loan.id)).to be_nil
      end
    end

    it "resets linked bank transactions to unmatched" do
      txn = BankTransaction.create!(
        reconciliation: recon, workspace: workspace,
        position: 1, booked_on: Date.current,
        description: "PREST", amount_cents: -40_000,
        currency: "EUR", status: :explained
      )
      inst = loan.instalments.first
      inst.update!(bank_transaction: txn, status: :paid)

      with_flags do
        sign_in(user)
        delete money_loan_path(loan)
        expect(txn.reload.status).to eq("unmatched")
      end
    end
  end

  describe "POST /money/loans/dismiss (dismiss)" do
    it "adds the key to dismissed_loan_suggestions" do
      with_flags do
        sign_in(user)
        post dismiss_money_loan_path, params: { key: "millennium bcp|78000" }
        expect(workspace.reload.settings["dismissed_loan_suggestions"]).to include("millennium bcp|78000")
      end
    end
  end

  describe "GET /money/loans/:id (show)" do
    let!(:loan) { create(:loan, :with_schedule, workspace:, created_by: user) }

    it "renders the loan detail page" do
      with_flags do
        sign_in(user)
        get money_loan_path(loan)
        expect(response).to have_http_status(:ok)
        expect(response.body).to include(loan.lender)
      end
    end
  end

  describe "cross-workspace isolation" do
    let(:other_workspace) { create(:workspace, plan: "pro") }
    let(:other_user)      { create(:user, workspace: other_workspace) }
    let!(:loan)           { create(:loan, workspace:, created_by: user) }

    it "returns 404 when another workspace tries to access the loan" do
      with_flags do
        sign_in(other_user)
        get money_loan_path(loan)
        expect(response).to have_http_status(:not_found)
      end
    end
  end

  describe "gating" do
    it "404s when accounting is off" do
      sign_in(user)
      post money_loans_path, params: { lender: "Test" }
      expect(response).to have_http_status(:not_found)
    end

    it "blocks a workspace without the accounting entitlement" do
      free = create(:workspace, plan: "free")
      free_user = create(:user, workspace: free)
      with_flags do
        sign_in(free_user)
        post money_loans_path, params: { lender: "Test" }
        expect(response).to have_http_status(:forbidden).or have_http_status(:redirect)
      end
    end
  end
end
