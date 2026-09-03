# frozen_string_literal: true

require "rails_helper"

# Guards the People index against per-counterpart query growth. The bold-layout
# People page used to build People::Standing once per person (and 5x per org),
# each rebuilding the awaiting-reply set and re-loading the person's threads — so
# the page's query count (and its ~26s prod load) scaled with the directory size.
# build_people_list now primes one shared Standing in a handful of batched loads,
# so adding people must NOT add queries.
RSpec.describe "People page performance", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before do
    allow(Features).to receive(:bold_layout?).and_return(true)
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
    sign_in(user)
  end

  # A person eligible for the directory: a person-kind contact with mail and one
  # inbound message on a thread — enough to exercise Standing's thread and
  # latest-inbound loads (the paths that used to run once per person).
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

  it "issues a bounded number of queries independent of the number of people" do
    3.times { |i| seed_person(i) }
    get people_path # prime per-request caches so the delta reflects only per-N work
    expect(response).to have_http_status(:ok)
    baseline = count_queries { get people_path }

    12.times { |i| seed_person(100 + i) } # 15 people total — 5x the baseline
    scaled = count_queries { get people_path }

    # If any per-person query survived, 12 more people would add ~12x that query.
    # The batched loads are flat, so allow only a tiny constant slack.
    expect(scaled).to be <= baseline + 3,
      "query count scaled with people (#{baseline} -> #{scaled}); a per-person query likely survived"
  end
end
