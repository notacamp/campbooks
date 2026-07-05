require "rails_helper"

# Pure-data reply-state scopes behind the "Waiting on replies" surfaces
# (inbox section, Scout count, feed, Skim — all via Emails::AwaitingReply).
# No AI, no provider calls — just the denormalized columns — so this is the
# CI-gating counterpart of the RSpec service specs.
RSpec.describe EmailThread, "awaiting reply" do
  before do
    @workspace = Workspace.create!(name: "Waiting WS")
    @account = EmailAccount.create!(workspace: @workspace, email_address: "me@example.com", refresh_token: "tok")
  end

  def thread(**attrs)
    EmailThread.create!({ email_account: @account, subject: "Re: project" }.merge(attrs))
  end

  it ".holds_last_word matches owner-sent-last threads and cold sends, not the rest" do
    held   = thread(last_outbound_at: 1.hour.ago, last_inbound_at: 2.hours.ago)
    cold   = thread(last_outbound_at: 1.hour.ago, last_inbound_at: nil)
    theirs = thread(last_outbound_at: 2.hours.ago, last_inbound_at: 1.hour.ago)
    never  = thread(last_outbound_at: nil, last_inbound_at: 1.hour.ago)

    result = EmailThread.holds_last_word
    expect(result).to include(held)
    expect(result).to include(cold)
    expect(result).not_to include(theirs)
    expect(result).not_to include(never)
  end

  it ".awaiting_reply excludes too-recent sends and dismissed threads" do
    due       = thread(last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago)
    recent    = thread(last_outbound_at: 1.hour.ago, last_inbound_at: 2.hours.ago)
    dismissed = thread(last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago, follow_up_dismissed_at: Time.current)

    result = EmailThread.awaiting_reply
    expect(result).to include(due)
    expect(result).not_to include(recent)
    expect(result).not_to include(dismissed)
  end

  it ".awaiting_reply vets with the AI verdict — drops 'no follow-up expected', keeps confirmed and unjudged" do
    unjudged  = thread(last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago)
    confirmed = thread(last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago,
                       follow_up_last_analyzed_at: 1.day.ago, follow_up_expected: true)
    fyi       = thread(last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago,
                       follow_up_last_analyzed_at: 1.day.ago, follow_up_expected: false)

    result = EmailThread.awaiting_reply
    expect(result).to include(unjudged)
    expect(result).to include(confirmed)
    expect(result).not_to include(fyi)
  end
end
