# frozen_string_literal: true

require "rails_helper"

RSpec.describe Feed::Sources::Task do
  # Pin the clock to a fixed midday so "due today" vs "overdue" (and the created_at
  # windows) never straddle midnight mid-run.
  around { |ex| travel_to(Time.utc(2026, 9, 7, 12, 0, 0)) { ex.run } }

  let(:workspace) { Workspace.create!(name: "Feed Task WS") }
  let(:user) do
    workspace.users.create!(
      name: "Reader", email_address: "reader-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end
  let(:account) { create(:email_account, workspace: workspace) }
  let(:source) { described_class.new(user) }

  def make_task(**attrs)
    ::Task.create!({ workspace: workspace, title: "T #{SecureRandom.hex(3)}", priority: :normal }.merge(attrs))
  end

  def candidate_for(task) = source.candidates.find { |c| c[:subject] == task }

  describe "the three candidate rules" do
    it "surfaces a confident suggestion with its own dedupe key + framing" do
      task = make_task(status: :suggested, ai_suggested: true, confidence: 0.9)
      c = candidate_for(task)

      expect(c[:dedupe_key]).to eq("task_suggestion:#{task.id}")
      expect(c[:data]["framing"]).to eq("suggested")
      expect(c[:score]).to eq(55)
      expect(c[:attention]).to be(false)
    end

    it "keeps low-confidence and stale suggestions off the feed" do
      low = make_task(status: :suggested, ai_suggested: true, confidence: 0.4)
      stale = make_task(status: :suggested, ai_suggested: true, confidence: 0.9)
      stale.update_column(:created_at, 20.days.ago)

      subjects = source.candidates.map { |c| c[:subject] }
      expect(subjects).not_to include(low)
      expect(subjects).not_to include(stale)
    end

    it "surfaces an accepted undated ask (framing undated, no attention)" do
      task = make_task(status: :todo, due_at: nil)
      c = candidate_for(task)

      expect(c[:dedupe_key]).to eq("task:#{task.id}")
      expect(c[:data]["framing"]).to eq("undated")
      expect(c[:score]).to eq(40)
      expect(c[:attention]).to be(false)
    end

    it "surfaces a due-today ask with attention" do
      task = make_task(status: :todo, due_at: 1.hour.from_now)
      c = candidate_for(task)

      expect(c[:data]["framing"]).to eq("due")
      expect(c[:score]).to eq(88)
      expect(c[:attention]).to be(true)
    end

    it "scores an overdue ask higher than a due-today one" do
      task = make_task(status: :todo, due_at: 2.days.ago)
      c = candidate_for(task)

      expect(c[:data]["framing"]).to eq("due")
      expect(c[:score]).to eq(92)
    end

    it "does not surface a future-dated ask (it lives on Time)" do
      task = make_task(status: :todo, due_at: 3.days.from_now)
      expect(candidate_for(task)).to be_nil
    end
  end

  it "suppresses a suggestion whose source email still has an active card" do
    email = create(:email_message, email_account: account)
    task = make_task(status: :suggested, ai_suggested: true, confidence: 0.9, source: email)
    FeedItem.create!(user: user, workspace: workspace, kind: "email_action", subject: email,
                     dedupe_key: "email_action:#{email.id}", sort_at: Time.current)

    expect(candidate_for(task)).to be_nil
  end

  it "surfaces the suggestion once the email card is gone" do
    email = create(:email_message, email_account: account)
    task = make_task(status: :suggested, ai_suggested: true, confidence: 0.9, source: email)

    expect(candidate_for(task)).to be_present
  end

  it "excludes snoozed asks entirely" do
    undated = make_task(status: :todo, due_at: nil, snoozed_until: 3.days.from_now)
    suggested = make_task(status: :suggested, ai_suggested: true, confidence: 0.9, snoozed_until: 3.days.from_now)

    subjects = source.candidates.map { |c| c[:subject] }
    expect(subjects).not_to include(undated)
    expect(subjects).not_to include(suggested)
  end

  describe "#still_valid?" do
    it "tracks the flavour: suggestions while suggested, actives while active + due" do
      task = make_task(status: :suggested, ai_suggested: true, confidence: 0.9)
      suggestion_item = FeedItem.new(dedupe_key: "task_suggestion:#{task.id}")
      active_item = FeedItem.new(dedupe_key: "task:#{task.id}")

      expect(source.still_valid?(suggestion_item, task)).to be_truthy
      expect(source.still_valid?(active_item, task)).to be_falsey

      task.move_to_status!(:todo, by: nil)
      expect(source.still_valid?(suggestion_item, task)).to be_falsey
      expect(source.still_valid?(active_item, task)).to be_truthy
    end

    it "is false for a nil, archived, or snoozed task, and a future-dated active ask" do
      task = make_task(status: :todo, due_at: nil)
      active_item = FeedItem.new(dedupe_key: "task:#{task.id}")

      expect(source.still_valid?(active_item, nil)).to be_falsey

      task.update!(snoozed_until: 2.days.from_now)
      expect(source.still_valid?(active_item, task)).to be_falsey

      task.update!(snoozed_until: nil, due_at: 5.days.from_now)
      expect(source.still_valid?(active_item, task)).to be_falsey

      task.update!(due_at: nil)
      task.archive!(by: nil)
      expect(source.still_valid?(active_item, task)).to be_falsey
    end
  end
end
