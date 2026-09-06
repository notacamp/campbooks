# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Feed::AskCard, type: :component do
  include Rails.application.routes.url_helpers

  around { |ex| travel_to(Time.utc(2026, 9, 7, 8, 0, 0)) { ex.run } } # Monday morning

  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  def make_task(**attrs)
    workspace.tasks.create!({ title: "Send the contract", status: :todo, priority: :normal }.merge(attrs))
  end

  def render_card(task, framing:)
    item = FeedItem.create!(user: user, workspace: workspace, kind: "task", subject: task,
                            dedupe_key: "task:#{task.id}", sort_at: Time.current, data: { "framing" => framing })
    Current.set(acting_user: user) do
      ApplicationController.render(described_class.new(item: item, subject: task), layout: false)
    end
  end

  it "suggested-undated: leads with Scout found an ask, offers Not now / Set a date / Hold" do
    task = make_task(status: :suggested, ai_suggested: true)
    html = render_card(task, framing: "suggested")

    expect(html).to include("Scout found an ask")
    expect(html).to include("no date")
    expect(html).to include("Not now")
    expect(html).to include("Set a date")
    expect(html).to include("Hold")
    expect(html).to include("is free for 45 minutes")
  end

  it "suggested-dated: shows the by-date" do
    task = make_task(status: :suggested, ai_suggested: true, due_at: 3.days.from_now)
    html = render_card(task, framing: "suggested")

    expect(html).to include("Scout found an ask")
    expect(html).to include("by ")
  end

  it "open-undated: shows no date and the three ways out" do
    task = make_task(status: :todo)
    html = render_card(task, framing: "undated")

    expect(html).to include("no date")
    expect(html).to include("Set a date")
    expect(html).to include("Not now")
  end

  it "due-today: shows Done and hides the date/hold controls" do
    task = make_task(status: :todo, due_at: 2.hours.from_now)
    html = render_card(task, framing: "due")

    expect(html).to include("due today")
    expect(html).to include("Done")
    expect(html).not_to include("Set a date")
  end

  it "overdue: shows the overdue kicker and Done" do
    task = make_task(status: :todo, due_at: 2.days.ago)
    html = render_card(task, framing: "due")

    expect(html).to include("overdue")
    expect(html).to include("Done")
  end

  it "links the title to the source email and offers Open thread + from-name on a due ask" do
    email = create(:email_message, email_account: account, from_address: "Rita Lopes <rita@acme.example>")
    task = make_task(status: :todo, due_at: 2.hours.from_now, source: email)
    html = render_card(task, framing: "due")

    expect(html).to include(email_message_path(email))
    expect(html).to include("Open thread")
    expect(html).to include("from Rita") # "from Rita's email" (apostrophe HTML-escaped)
  end
end
