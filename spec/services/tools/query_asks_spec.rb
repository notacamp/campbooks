# frozen_string_literal: true

require "rails_helper"

RSpec.describe Tools::QueryAsks do
  around { |ex| travel_to(Time.utc(2026, 9, 7, 12, 0, 0)) { ex.run } }

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  before do
    Current.acting_user = user
    Current.workspace = workspace
    allow(Features).to receive(:tasks?).and_return(true)
  end

  def ask(**attrs)
    workspace.tasks.create!({ title: "Owe #{SecureRandom.hex(2)}", status: :todo, priority: :normal }.merge(attrs))
  end

  it "returns the user's live asks with cards (kind ask, capped at 10)" do
    12.times { ask(status: :todo, due_at: 1.day.from_now) }

    result = described_class.call({})

    expect(result[:count]).to eq(12)
    expect(result[:asks].size).to eq(12)
    expect(result[:cards].size).to eq(10)
    expect(result[:cards]).to all(include("kind" => "ask"))
    expect(result[:cards].first).to include("id", "title", "meta")
  end

  it "filters to undated asks" do
    dated = ask(due_at: 1.day.from_now)
    undated = ask(due_at: nil)

    ids = described_class.call({ "undated" => true })[:asks].map { |a| a[:id] }
    expect(ids).to include(undated.id)
    expect(ids).not_to include(dated.id)
  end

  it "filters by due_within_days" do
    soon = ask(due_at: 2.days.from_now)
    far  = ask(due_at: 30.days.from_now)

    ids = described_class.call({ "due_within_days" => 7 })[:asks].map { |a| a[:id] }
    expect(ids).to include(soon.id)
    expect(ids).not_to include(far.id)
  end

  it "handed:true returns asks handed to others, which for_user excludes" do
    other = create(:user, workspace: workspace)
    task = ask(status: :todo)
    Asks::HandOff.call(task, to: other, by: user)

    default_ids = described_class.call({})[:asks].map { |a| a[:id] }
    handed_ids  = described_class.call({ "handed" => true })[:asks].map { |a| a[:id] }

    expect(default_ids).not_to include(task.id)
    expect(handed_ids).to include(task.id)
    expect(described_class.call({ "handed" => true })[:asks].first[:handed_to]).to eq(other.name)
  end

  it "is empty for no acting user" do
    Current.acting_user = nil
    expect(described_class.call({})).to eq({ count: 0, asks: [], cards: [] })
  end

  it "is empty when tasks are gated off" do
    allow(Features).to receive(:tasks?).and_return(false)
    ask(status: :todo)
    expect(described_class.call({})[:count]).to eq(0)
  end
end
