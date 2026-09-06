# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Money::Strip, type: :component do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:today)     { Date.new(2024, 2, 15) }

  def build_read(workspace: self.workspace, user: self.user, today: self.today)
    ev  = Money::Evidence.for(workspace, today: today)
    led = Money::Ledger.for(workspace, user, today: today, evidence: ev)
    Money::Read.for(workspace, user, today: today, evidence: ev, ledger: led)
  end

  def render_component(read)
    ApplicationController.render(described_class.new(read: read), layout: false)
  end

  context "with no statements" do
    it "renders nothing" do
      html = render_component(build_read)
      expect(html.strip).to be_empty
    end
  end

  context "with a ready statement" do
    let!(:statement) do
      create(:reconciliation, :ready, :with_bank, workspace: workspace,
             period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
    end

    it "shows the explained stat" do
      html = render_component(build_read)
      expect(html).to include("explained")
    end

    it "hides the zero stats when nothing needs an invoice and nothing is missing" do
      html = render_component(build_read)
      expect(html).not_to include("money_needs")
      expect(html).not_to include("money_unbanked")
    end

    it "leads with the month to reconcile while its statement isn't in" do
      html = render_component(build_read(today: Date.new(2024, 3, 15)))
      expect(html).to include("February")
      expect(html).to include("No statement yet")
      expect(html).to include(Rails.application.routes.url_helpers.new_reconciliation_path)
      expect(html.index("February")).to be < html.index("explained")
    end

    it "sends the month to reconcile to Needs-you when Scout is holding statements" do
      create(:document, :bank_statement, :approved, workspace: workspace)
      html = render_component(build_read(today: Date.new(2024, 3, 15)))
      expect(html).to include("No statement yet")
      expect(html).to include("#money_needs")
    end

    it "drops the month stat once its statement is in" do
      html = render_component(build_read)
      expect(html).not_to include("No statement yet")
    end

    it "links an unexplained debit to the Needs-you section and a missing invoice to the unbanked list" do
      create(:bank_transaction, reconciliation: statement, workspace: workspace, status: :unmatched,
             amount_cents: -20_000, booked_on: Date.new(2024, 1, 28))
      create(:document, :approved, workspace: workspace, document_type: :expense_invoice,
             amount_cents: 8_860, currency: "EUR", document_date: Date.new(2024, 1, 22))

      html = render_component(build_read)
      expect(html).to include("#money_needs")
      expect(html).to include("#money_unbanked")
      expect(html).to include("€200.00")
    end
  end
end
