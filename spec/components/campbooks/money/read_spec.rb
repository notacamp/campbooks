# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Money::Read, type: :component do
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
    it "shows the no-statements message with a link to add one" do
      html = render_component(build_read)
      expect(html).to include("No bank statements yet")
      expect(html).to include("Scout")
    end

    it "points at the statements Scout is holding instead when there are some" do
      create(:document, :bank_statement, :approved, workspace: workspace)
      html = render_component(build_read)
      expect(html).to include("I&#39;m holding a bank statement from your email").or include("I'm holding a bank statement from your email")
      expect(html).to include("#money_needs")
      expect(html).not_to include("No bank statements yet")
    end
  end

  context "with the month to reconcile still open" do
    let!(:jan_stmt) do
      create(:reconciliation, :ready, :with_bank, workspace: workspace,
             period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
    end

    it "says that month's statement isn't in yet, before the newest statement" do
      html     = render_component(build_read(today: Date.new(2024, 3, 15)))
      sentence = html[%r{<p class=[^>]*>(.*?)</p>}m, 1]
      expect(sentence).to match(/February(&#39;|')s statement isn(&#39;|')t in yet/)
      expect(sentence.index("February")).to be < sentence.index("January")
    end

    it "asks to reconcile the statements Scout is holding instead" do
      create(:document, :bank_statement, :approved, workspace: workspace)
      html = render_component(build_read(today: Date.new(2024, 3, 15)))
      expect(html).to include("Reconcile it?")
      expect(html).not_to match(/statement isn(&#39;|')t in yet/)
    end

    it "counts missing invoices across every reconciled month, not just the newest statement" do
      create(:reconciliation, :ready, :with_bank, workspace: workspace,
             period_start: Date.new(2023, 12, 1), period_end: Date.new(2023, 12, 31))
      create(:document, :approved, workspace: workspace, document_type: :expense_invoice,
             amount_cents: 10_000, currency: "EUR", document_date: Date.new(2024, 1, 10))
      create(:document, :approved, workspace: workspace, document_type: :expense_invoice,
             amount_cents: 10_000, currency: "EUR", document_date: Date.new(2023, 12, 10))
      html = render_component(build_read)
      expect(html).to match(/2 invoices from reconciled months aren(&#39;|')t on a statement/)
    end
  end

  context "with a ready statement" do
    let!(:jan_stmt) do
      create(:reconciliation, :ready, :with_bank, workspace: workspace,
             period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
    end

    it "shows statement context" do
      html = render_component(build_read)
      expect(html).to include("January")
      expect(html).to include("Scout")
    end

    it "does not contain forbidden words" do
      html = render_component(build_read)
      %w[owe owed late overdue].each do |word|
        expect(html.downcase).not_to include(word)
      end
    end

    it "reads naturally when the statement has no bank name" do
      jan_stmt.update!(bank_name: nil)
      html = render_component(build_read)
      expect(html).to match(/January(&#39;|')s statement is in/)
      expect(html).not_to include("the bank")
    end

    it "shows all explained message when total equals explained" do
      create(:bank_transaction, reconciliation: jan_stmt, workspace: workspace,
             status: :matched)
      html = render_component(build_read)
      expect(html).to include("Every line has its paper")
    end
  end
end
