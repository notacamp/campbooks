# frozen_string_literal: true

require "rails_helper"

RSpec.describe "Paper", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def paper_doc(*traits, **attrs)
    create(:document, :approved, *traits, workspace: workspace, tax_amount_cents: nil, tax_rate: nil, **attrs)
  end

  describe "the flag gate" do
    it "404s when the bold layout flag is off, even signed in" do
      sign_in(user)
      get paper_path
      expect(response).to have_http_status(:not_found)
    end
  end

  context "with the bold layout flag on" do
    before do
      allow(Features).to receive(:bold_layout?).and_return(true)
      sign_in(user)
    end

    it "renders each document as a row of facts + a derived status" do
      paper_doc(document_type: :expense_invoice, amount_cents: 24_800, currency: "EUR",
               due_date: Date.current + 5, vendor_name: "Cloudhost")
      statement = paper_doc(:bank_statement, bank_name: "Millennium BCP")
      create(:reconciliation, :ready, statement_document: statement, workspace: workspace)

      get paper_path

      expect(response).to have_http_status(:ok)
      expect(response.body).to include("€248.00")     # the bold amount fact
      expect(response.body).to include("Unpaid")       # the derived status chip
      expect(response.body).to include("Reconciled")   # the statement's status
      expect(response.body).to include("paper_results")
    end

    it "filters by type bucket" do
      paper_doc(document_type: :expense_invoice, amount_cents: 1_000, due_date: Date.current + 3,
               vendor_name: "Acme Invoicing")
      paper_doc(:receipt, amount_cents: 500, vendor_name: "Corner Shop")

      get paper_path(type: :receipts)

      expect(response.body).to include("Corner Shop")
      expect(response.body).not_to include("Acme Invoicing")
    end

    it "post-filters by a derived status word in the ask box" do
      paper_doc(document_type: :expense_invoice, amount_cents: 1_000, due_date: Date.current + 3,
               vendor_name: "Still Open Ltd")
      paper_doc(document_type: :expense_invoice, amount_cents: 2_000, due_date: Date.current - 3,
               settled_at: Time.current, settled_source: "manual", vendor_name: "Settled Ltd")

      get paper_path(q: "unpaid")

      expect(response.body).to include("Still Open Ltd")
      expect(response.body).not_to include("Settled Ltd")
    end

    it "offers the Export and Add affordances" do
      get paper_path
      expect(response.body).to include(export_documents_path)
      expect(response.body).to include(I18n.t("paper.index.add"))
    end
  end

  describe "settle / unsettle" do
    let(:document) do
      paper_doc(document_type: :expense_invoice, amount_cents: 9_600, due_date: Date.current - 2)
    end

    before do
      allow(Features).to receive(:bold_layout?).and_return(true)
      sign_in(user)
    end

    it "marks a document paid (manual) and back to unpaid" do
      post settle_document_path(document)
      expect(document.reload).to be_settled
      expect(document.settled_source).to eq("manual")

      delete unsettle_document_path(document)
      expect(document.reload.settled_at).to be_nil
    end

    it "404s settling a document in another workspace" do
      foreign = create(:document, :approved, document_type: :expense_invoice, amount_cents: 100)
      post settle_document_path(foreign)
      expect(response).to have_http_status(:not_found)
      expect(foreign.reload.settled_at).to be_nil
    end
  end
end
