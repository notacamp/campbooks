# frozen_string_literal: true

require "rails_helper"

# The month pills: the month to reconcile first, every month back to the first
# statement, gaps folded, the selected statement marked.
RSpec.describe Campbooks::Money::StatementTabs, type: :component do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:today)     { Date.new(2026, 9, 6) }

  def statement(from, to)
    create(:reconciliation, :ready, workspace: workspace, created_by: user, bank_name: "Millennium BCP",
           period_start: from, period_end: to)
  end

  let!(:jun)   { statement(Date.new(2025, 6, 1), Date.new(2025, 6, 30)) }
  let!(:jul_a) { statement(Date.new(2025, 7, 1), Date.new(2025, 7, 31)) }
  let!(:jul_b) { statement(Date.new(2025, 7, 1), Date.new(2025, 7, 31)) }
  let!(:nov)   { statement(Date.new(2025, 11, 1), Date.new(2025, 11, 30)) }

  def render_tabs(statement_id: nil)
    page = Money::Page.for(workspace, user, today: today, statement_id: statement_id)
    ApplicationController.render(described_class.new(page: page), layout: false)
  end

  it "leads with the month to reconcile and folds the months without a statement" do
    html = render_tabs
    expect(html).to include("August · no statement yet")
    expect(html).to include("Dec 2025 to Jul 2026 · 8 months without a statement")
    expect(html).to include("November 2025")
    expect(html).to include("Aug 2025 to Oct 2025 · 3 months without a statement")
    expect(html.scan("July 2025").size).to eq(2)
    expect(html).to include("June 2025")
    expect(html.index("August · no statement yet")).to be < html.index("November 2025")
    expect(html.index("November 2025")).to be < html.index("June 2025")
  end

  it "targets the statement frame from every statement pill and the reconcile page from a gap" do
    html = render_tabs
    expect(html).to include(%(href="/money?statement=#{nov.id}"))
    expect(html.scan('data-turbo-frame="money_statement"').size).to eq(4)
    expect(html).to include(Rails.application.routes.url_helpers.new_reconciliation_path)
  end

  it "marks the newest statement selected by default and the clicked one after" do
    expect(render_tabs).to match(/statement=#{nov.id}"[^>]*aria-selected="true"/)

    html = render_tabs(statement_id: jun.id)
    expect(html).to match(/statement=#{jun.id}"[^>]*aria-selected="true"/)
    expect(html).to match(/statement=#{nov.id}"[^>]*aria-selected="false"/)
  end

  it "shows the month alone when it is this year" do
    stmt = statement(Date.new(2026, 8, 1), Date.new(2026, 8, 31))
    html = render_tabs(statement_id: stmt.id)
    expect(html).to include(">August<").or include("August</a>").or include("August")
    expect(html).not_to include("August 2026")
    expect(html).not_to include("no statement yet")
  end
end
