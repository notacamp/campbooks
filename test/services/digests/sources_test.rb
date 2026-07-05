# frozen_string_literal: true

require "test_helper"

class Digests::SourcesTest < ActiveSupport::TestCase
  setup do
    @ws   = Workspace.create!(name: "Sources WS")
    @user = @ws.users.create!(name: "Srcs", email_address: "srcs@example.com", password: "password123")
    @digest = @ws.scheduled_digests.create!(
      user:        @user,
      name:        "Sources test",
      rrule:       "FREQ=WEEKLY",
      next_run_at: 1.week.from_now,
      config:      { "sources" => [ { "type" => "emails", "query" => "" } ] }
    )
  end

  def make_source(type, config = {})
    Digests::Sources.for(type).new(@digest, config)
  end

  # ── Emails source — permission visibility ─────────────────────────────────────

  test "emails source only returns messages on accounts the user can read" do
    # Account A: user CAN read
    acct_a = EmailAccount.create!(workspace: @ws, email_address: "me@example.com", refresh_token: "tok_a")
    @user.email_account_users.create!(email_account: acct_a, can_read: true, can_send: false)
    msg_a = EmailMessage.create!(
      email_account:       acct_a,
      from_address:        "sender@example.com",
      to_address:          "me@example.com",
      subject:             "Accessible email",
      provider_message_id: SecureRandom.hex(8),
      provider_folder_id:  "INBOX",
      received_at:         1.day.ago,
      status:              :processed
    )

    # Account B: user CANNOT read
    other_user = @ws.users.create!(name: "Other", email_address: "other@example.com", password: "password123")
    acct_b = EmailAccount.create!(workspace: @ws, email_address: "other@example.com", refresh_token: "tok_b")
    other_user.email_account_users.create!(email_account: acct_b, can_read: true, can_send: false)
    msg_b = EmailMessage.create!(
      email_account:       acct_b,
      from_address:        "sender@example.com",
      to_address:          "other@example.com",
      subject:             "Inaccessible email",
      provider_message_id: SecureRandom.hex(8),
      provider_folder_id:  "INBOX",
      received_at:         1.day.ago,
      status:              :processed
    )

    period = 2.days.ago..Time.current
    source = make_source("emails", { "query" => "" })
    items = source.items(period)

    ids = items.map(&:source_id)
    assert_includes ids, msg_a.id
    assert_not_includes ids, msg_b.id
  end

  # ── tasks source — gated when tasks feature is off ───────────────────────────

  test "available_keys excludes tasks when ENABLE_TASKS is off and entitlement missing" do
    with_env("ENABLE_TASKS" => nil) do
      keys = Digests::Sources.available_keys(@ws)
      assert_not_includes keys, "tasks"
    end
  end

  test "available_keys includes tasks when ENABLE_TASKS is on and entitlement granted" do
    with_env("ENABLE_TASKS" => "1") do
      @ws.update!(entitlement_overrides: { "tasks" => { "allowed" => true, "enabled" => true } })
      keys = Digests::Sources.available_keys(@ws)
      assert_includes keys, "tasks"
    end
  end

  # ── Source resolution ─────────────────────────────────────────────────────────

  test "for returns nil for unknown type" do
    assert_nil Digests::Sources.for("unknown")
  end

  test "KEYS includes all expected source types" do
    %w[emails calendar tasks reminders documents].each do |key|
      assert_includes Digests::Sources::KEYS, key
    end
  end

  # ── Reminders source ─────────────────────────────────────────────────────────

  test "reminders source excludes task-sourced reminders" do
    # Create a task and its reminder
    task = @ws.tasks.create!(title: "A task", status: :todo, priority: :normal, confidence: 1.0)
    src_msg_for_reminder
    task_reminder = Reminder.create!(
      workspace:     @ws,
      source:        task,
      title:         "Task reminder",
      due_at:        6.days.from_now,
      reminder_type: :deadline,
      status:        :pending,
      confidence:    0.9
    )

    period = Time.current..(Time.current + 7.days)
    source = make_source("reminders", { "window_days" => 7 })
    items = source.items(period)

    assert_not_includes items.map(&:source_id), task_reminder.id
  end

  private

  def src_msg_for_reminder
    acct = EmailAccount.create!(workspace: @ws, email_address: "srcs@example.com", refresh_token: "tok_srcs")
    @user.email_account_users.create!(email_account: acct, can_read: true, can_send: false)
    thread = EmailThread.create!(email_account: acct, subject: "Invoice")
    EmailMessage.create!(email_account: acct, email_thread: thread,
                         from_address: "v@example.com", to_address: "srcs@example.com",
                         subject: "Invoice", provider_folder_id: "INBOX",
                         received_at: 1.day.ago, provider_message_id: SecureRandom.hex(8),
                         status: :processed)
  end
end
