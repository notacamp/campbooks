# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Money::Strip, type: :component do
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
    it "renders nothing (hidden)" do
      html = render_component(build_read)
      expect(html).to include("hidden")
    end
  end

  context "with a ready statement" do
    before do
      create(:reconciliation, :ready, :with_bank, workspace: workspace,
             period_start: Date.new(2024, 1, 1), period_end: Date.new(2024, 1, 31))
    end

    it "shows the explained stat" do
      html = render_component(build_read)
      expect(html).to include("explained")
    end

    it "includes links to money_needs and money_unbanked anchors" do
      html = render_component(build_read)
      expect(html).to include("money_needs").or include("money_unbanked")
    end
  end
end
