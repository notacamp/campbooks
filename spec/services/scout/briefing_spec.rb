# frozen_string_literal: true

require "rails_helper"

RSpec.describe Scout::Briefing do
  let(:user) { create(:user) }

  before { allow(Features).to receive(:tasks?).and_return(true) }

  it "prepends the owe suggestion when the user has a live ask" do
    user.workspace.tasks.create!(title: "Send the file", status: :todo, priority: :normal)

    suggestions = described_class.for(user)[:suggestions]
    expect(suggestions.first).to eq(I18n.t("scout.briefing.suggestions.owe"))
  end

  it "omits the owe suggestion when there is no live ask" do
    suggestions = described_class.for(user)[:suggestions]
    expect(suggestions).not_to include(I18n.t("scout.briefing.suggestions.owe"))
  end

  it "omits the owe suggestion when tasks are gated off" do
    allow(Features).to receive(:tasks?).and_return(false)
    user.workspace.tasks.create!(title: "Send the file", status: :todo, priority: :normal)

    suggestions = described_class.for(user)[:suggestions]
    expect(suggestions).not_to include(I18n.t("scout.briefing.suggestions.owe"))
  end
end
