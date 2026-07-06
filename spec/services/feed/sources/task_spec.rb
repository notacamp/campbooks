# frozen_string_literal: true

require "rails_helper"

RSpec.describe Feed::Sources::Task do
  let(:workspace) { Workspace.create!(name: "Feed Task WS") }
  let(:user) do
    workspace.users.create!(
      name: "Reader", email_address: "reader-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end
  let(:source) { described_class.new(user) }

  def suggested_task(confidence:)
    ::Task.create!(
      workspace: workspace, title: "Suggested #{SecureRandom.hex(3)}",
      status: :suggested, priority: :normal, ai_suggested: true, confidence: confidence
    )
  end

  it "confident suggestions surface with their own dedupe key" do
    task = suggested_task(confidence: 0.9)

    candidate = source.candidates.find { |c| c[:subject] == task }

    expect(candidate).to be_truthy, "expected a candidate for the suggestion"
    expect(candidate[:dedupe_key]).to eq("task_suggestion:#{task.id}")
    expect(candidate[:data]["status"]).to eq("suggested")
  end

  it "low-confidence and stale suggestions stay off the feed" do
    low = suggested_task(confidence: 0.4)
    stale = suggested_task(confidence: 0.9)
    stale.update_column(:created_at, 20.days.ago)

    subjects = source.candidates.map { |c| c[:subject] }

    expect(subjects).not_to include(low)
    expect(subjects).not_to include(stale)
  end

  it "active tasks keep the plain task dedupe key" do
    task = ::Task.create!(workspace: workspace, title: "Ship it", status: :todo,
                          priority: :normal, due_at: 1.day.from_now)

    candidate = source.candidates.find { |c| c[:subject] == task }

    expect(candidate).to be_truthy
    expect(candidate[:dedupe_key]).to eq("task:#{task.id}")
  end

  it "still_valid? tracks the flavor: suggestions while suggested, actives while active" do
    task = suggested_task(confidence: 0.9)
    suggestion_item = FeedItem.new(dedupe_key: "task_suggestion:#{task.id}")
    active_item = FeedItem.new(dedupe_key: "task:#{task.id}")

    expect(source.still_valid?(suggestion_item, task)).to be_truthy
    expect(source.still_valid?(active_item, task)).to be_falsey

    task.move_to_status!(:todo, by: nil)
    expect(source.still_valid?(suggestion_item, task)).to be_falsey
    expect(source.still_valid?(active_item, task)).to be_truthy

    task.archive!(by: nil)
    expect(source.still_valid?(active_item, task)).to be_falsey
    expect(source.still_valid?(suggestion_item, nil)).to be_falsey
  end
end
