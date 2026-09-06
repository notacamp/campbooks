# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Money::Read, type: :component do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:today)     { Date.new(2024, 2, 15) }

  def build_read(workspace: self.workspace, user: self.user)
    ev  = Money::Evidence.for(workspace)
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

    it "shows all explained message when total equals explained" do
      create(:bank_transaction, reconciliation: jan_stmt, workspace: workspace,
             status: :matched)
      html = render_component(build_read)
      expect(html).to include("Every line has its paper")
    end
  end
end
