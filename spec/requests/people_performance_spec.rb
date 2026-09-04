# frozen_string_literal: true

require "rails_helper"

# Guards the People index against per-counterpart query growth. After
# materializing the standings table, GET /people reads a paginated table slice
# and must never touch email_messages or email_threads. Adding more people to
# the directory should not increase the query count at request time.
RSpec.describe "People page performance", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    allow(Features).to receive(:bold_layout?).and_return(true)
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
    sign_in(user)
  end

  # A person eligible for the directory with a person-kind contact and one
  # inbound message.
  def seed_person(i)
    person = create(:person, workspace: workspace, name: "Person #{i}", context_summary: nil)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     name: "Person #{i}", email: "person#{i}@example.test",
                     sender_kind: :person, sender_kind_source: "heuristic")
    thread = create(:email_thread, email_account: account, subject: "Thread #{i}")
    create(:email_message, email_account: account, email_thread: thread, contact: contact,
           from_address: contact.email, subject: "Msg #{i}", received_at: (i + 1).hours.ago)
    contact.update_columns(email_count: 2, last_email_at: (i + 1).hours.ago)
    person
  end

  def refresh_standings!
    People::Standings.refresh!(user)
  end

  def count_queries
    count = 0
    sub = ActiveSupport::Notifications.subscribe("sql.active_record") do |*, payload|
      count += 1 unless payload[:cached] || %w[SCHEMA TRANSACTION].include?(payload[:name])
    end
    yield
    count
  ensure
    ActiveSupport::Notifications.unsubscribe(sub)
  end

  def sql_log
    sqls = []
    sub = ActiveSupport::Notifications.subscribe("sql.active_record") do |*, payload|
      sqls << payload[:sql] unless payload[:cached] || %w[SCHEMA TRANSACTION].include?(payload[:name])
    end
    yield
    sqls
  ensure
    ActiveSupport::Notifications.unsubscribe(sub)
  end

  # Queries fired on every bold-layout page (topbar indicators) — not People-specific.
  # These are expected and stable: unread count, reminders, agent messages, documents badge,
  # notifications count/exists, draft reminder, email account status.
  GLOBAL_LAYOUT_SQL_PATTERNS = [
    '"viewed_at" IS NULL',   # unread email / document badge checks
    'FROM "reminders"',      # reminders indicator (subqueries email_messages)
    'FROM "agent_messages"', # Scout indicator
    'FROM "notifications"',  # notification bell
    'FROM "draft_emails"'    # draft reminder chip
  ].freeze

  it "issues at most 22 non-cached queries after standings are refreshed" do
    3.times { |i| seed_person(i) }
    refresh_standings!

    # 12 People-specific queries (session + user/workspace + contacts check + standings x4 +
    # accounts + tags + inbox rules + feed items) plus up to 10 global topbar queries.
    # The frame request is the list path proper: the full HTML page also opens the top
    # row's conversation (a bounded extra, guarded below). The sendable_account_ids pluck
    # only fires on the full conversation page (show action), not the list frame.
    count = count_queries { get people_path, headers: { "Turbo-Frame" => "people_results" } }
    expect(response).to have_http_status(:ok)
    expect(count).to be <= 22,
      "expected <= 22 queries, got #{count}"
  end

  it "issues NO People-list query against email_messages or email_threads at request time" do
    3.times { |i| seed_person(i) }
    refresh_standings!

    sqls = sql_log { get people_path, headers: { "Turbo-Frame" => "people_results" } }
    # Exclude known global layout queries that touch email_messages only as a subquery
    # (topbar unread count, reminders indicator) — those are fired on every bold page.
    people_sqls = sqls.reject { |sql| GLOBAL_LAYOUT_SQL_PATTERNS.any? { |pat| sql.include?(pat) } }
    bad = people_sqls.select { |sql|
      sql.include?('FROM "email_messages"') || sql.include?('FROM "email_threads"')
    }
    expect(bad).to be_empty,
      "expected no People-list email_messages/email_threads queries, got:\n#{bad.join("\n")}"
  end

  it "keeps the full page, which opens the top row's conversation, bounded" do
    3.times { |i| seed_person(i) }
    EmailMessage.update_all(read: true, viewed_at: Time.current) # steady state: nothing left to mark read
    refresh_standings!

    count = count_queries { get people_path }
    expect(response).to have_http_status(:ok)
    # The auto-opened conversation is present (no "Open in inbox" — People is the inbox)
    expect(response.body).not_to include("Open in inbox")
    # The list (above) + one thread's messages, its heads, the nothing-to-mark-read
    # check, Scout's draft lookup, the sendable_account_ids pluck (+1) and the
    # conversation header — flat in the history.
    expect(count).to be <= 46, "expected <= 46 queries for the page with the top row open, got #{count}"
  end

  it "keeps the first open of unread mail, which marks it read and refreshes the row, bounded" do
    3.times { |i| seed_person(i) }
    refresh_standings!

    count = count_queries { get people_path }
    expect(response).to have_http_status(:ok)
    # Marking read updates the thread, broadcasts its inbox row, recomputes this one
    # person's standing and swaps it into the list: a fixed extra, once per unread open.
    # The sendable_account_ids pluck adds one query (+1 vs. prior budget).
    expect(count).to be <= 76, "expected <= 76 queries for the first open of unread mail, got #{count}"
    expect(count_queries { get people_path }).to be <= 46 # and the page is back in steady state
  end

  it "query count is flat when the directory grows (scaling assertion)" do
    3.times { |i| seed_person(i) }
    refresh_standings!
    get people_path # warm per-request caches
    baseline = count_queries { get people_path }

    12.times { |i| seed_person(100 + i) }
    refresh_standings!
    scaled = count_queries { get people_path }

    expect(scaled).to be <= baseline,
      "query count scaled with people (#{baseline} -> #{scaled}); a per-row query likely survived"
  end
end
