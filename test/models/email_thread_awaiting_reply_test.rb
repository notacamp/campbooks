require "test_helper"

# Pure-data reply-state scopes behind the "Waiting on replies" surfaces
# (inbox section, Scout count, feed, Skim — all via Emails::AwaitingReply).
# No AI, no provider calls — just the denormalized columns — so this is the
# CI-gating counterpart of the RSpec service specs.
class EmailThreadAwaitingReplyTest < ActiveSupport::TestCase
  setup do
    @workspace = Workspace.create!(name: "Waiting WS")
    @account = EmailAccount.create!(workspace: @workspace, email_address: "me@example.com", refresh_token: "tok")
  end

  def thread(**attrs)
    EmailThread.create!({ email_account: @account, subject: "Re: project" }.merge(attrs))
  end

  test ".holds_last_word matches owner-sent-last threads and cold sends, not the rest" do
    held   = thread(last_outbound_at: 1.hour.ago, last_inbound_at: 2.hours.ago)
    cold   = thread(last_outbound_at: 1.hour.ago, last_inbound_at: nil)
    theirs = thread(last_outbound_at: 2.hours.ago, last_inbound_at: 1.hour.ago)
    never  = thread(last_outbound_at: nil, last_inbound_at: 1.hour.ago)

    result = EmailThread.holds_last_word
    assert_includes result, held
    assert_includes result, cold
    assert_not_includes result, theirs
    assert_not_includes result, never
  end

  test ".awaiting_reply excludes too-recent sends and dismissed threads" do
    due       = thread(last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago)
    recent    = thread(last_outbound_at: 1.hour.ago, last_inbound_at: 2.hours.ago)
    dismissed = thread(last_outbound_at: 1.day.ago, last_inbound_at: 2.days.ago, follow_up_dismissed_at: Time.current)

    result = EmailThread.awaiting_reply
    assert_includes result, due
    assert_not_includes result, recent
    assert_not_includes result, dismissed
  end
end
