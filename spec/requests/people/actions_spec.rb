# frozen_string_literal: true

require "rails_helper"

RSpec.describe "People::Actions", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  def grant_access(can_send: true)
    eau = EmailAccountUser.find_or_initialize_by(user: user, email_account: account)
    eau.assign_attributes(can_read: true, can_send: can_send)
    eau.save!
  end

  def make_person(name:, email:, inbound_at: 2.days.ago, owe: false)
    person  = create(:person, workspace: workspace, name: name)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                     name: name, email: email, sender_kind: :person, sender_kind_source: "heuristic")
    thread  = create(:email_thread, email_account: account, subject: "Thread #{name}")
    msg     = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                     from_address: email, subject: "Msg #{name}", received_at: inbound_at)
    contact.update_columns(email_count: 1, last_email_at: inbound_at)
    thread.update_columns(last_inbound_at: inbound_at) if owe
    [ person, contact, thread, msg ]
  end

  def make_feed_item(user:, person:, contact:, thread:, msg:, kind: "reply_reminder")
    FeedItem.create!(user: user, workspace: workspace,
                     subject: msg,
                     kind: kind,
                     score: 0.8,
                     dedupe_key: "#{kind}:#{msg.id}",
                     sort_at: Time.current,
                     data: { "thread_id" => thread.id, "message_id" => msg.id })
  end

  def refresh_standings!
    People::Standings.refresh!(user)
  end

  # After refresh!, feed_item_id is nil because still_valid? requires ai_action_prompt /
  # holds_last_word? conditions that are hard to satisfy in unit tests.
  # Patch the standing row directly so the controller can find the item.
  def link_feed_item!(person, item, msg)
    PeopleStanding.for_user(user)
                  .where(counterpart: person)
                  .update_all(feed_item_id: item.id, email_message_id: msg.id)
  end

  # Patch just the email_message_id so message-based actions (snooze/archive) work.
  def link_message!(person, msg)
    PeopleStanding.for_user(user)
                  .where(counterpart: person)
                  .update_all(email_message_id: msg.id)
  end

  before do
    grant_access
    sign_in(user)
  end

  # ── 404 guards ──────────────────────────────────────────────────────────────

  it "404s when the row belongs to a different user" do
    other_user = create(:user, workspace: workspace)
    person, = make_person(name: "Sofia", email: "sofia@x.example")
    # Build the standing for other_user, not current user.
    People::Standings.refresh!(other_user)

    post people_action_path(person.id, :done),
         headers: { "Accept" => "text/vnd.turbo-stream.html" }
    expect(response).to have_http_status(:not_found)
  end

  it "404s for the done action on an organization row" do
    org = create(:organization, workspace: workspace, name: "ACME")
    person, contact = make_person(name: "Rui", email: "rui@acme.example")
    create(:organization_membership, person: person, organization: org)
    # Organizations don't get feed items; their row lacks feed_item_id.
    refresh_standings!

    post people_action_path(org.id, :done),
         headers: { "Accept" => "text/vnd.turbo-stream.html" }
    # org row would be missing for the user since orgs require a money attention item
    expect(response).to have_http_status(:not_found)
  end

  # ── paid ────────────────────────────────────────────────────────────────────

  context "paid" do
    # A late expense invoice from this person: the feed's late_payable item is
    # real (it passes still_valid?), so refresh! projects it onto the row as Pay.
    def make_late_invoice(contact:, msg:)
      doc = create(:document, workspace: workspace, document_type: :expense_invoice,
                              ai_status: :completed, review_status: :pending)
      doc.update!(metadata: (doc.metadata || {}).merge(
        "amount_cents" => 24_800, "currency" => "EUR",
        "due_date" => 20.days.ago.to_date.iso8601, "invoice_number" => "FT2026/2756"
      ))
      DocumentEmailMessage.create!(document: doc, email_message: msg)
      item = FeedItem.create!(user: user, workspace: workspace, subject: doc, kind: "late_payable",
                              score: 90.0, dedupe_key: "late_payable:#{doc.id}", sort_at: 20.days.ago,
                              attention: true,
                              data: { "due_date" => 20.days.ago.to_date.iso8601, "days_late" => 20,
                                      "amount_cents" => 24_800, "currency" => "EUR" })
      [ doc, item ]
    end

    it "settles the invoice, retires the card, and re-renders the list" do
      person, contact, _thread, msg = make_person(name: "Cloudhost Billing", email: "billing@cloudhost.example")
      doc, item = make_late_invoice(contact: contact, msg: msg)
      refresh_standings!
      row = PeopleStanding.for_user(user).find_by(counterpart: person)
      expect(row.verb).to eq("pay")
      expect(row.feed_item_id).to eq(item.id)

      post people_action_path(person.id, :paid),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(doc.reload).to be_settled
      expect(item.reload.dismissed_at).to be_present
      expect(response.body).to include('<turbo-stream action="update" target="people_results"')
      expect(response.body).to include("marked paid").or include("invoice paid")
    end

    it "404s when the row's item is not a money item" do
      person, contact, thread, msg = make_person(name: "Sofia", email: "sofia@x.example", owe: true)
      item = make_feed_item(user: user, person: person, contact: contact,
                            thread: thread, msg: msg, kind: "reply_reminder")
      refresh_standings!
      link_feed_item!(person, item, msg)

      post people_action_path(person.id, :paid),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:not_found)
    end
  end

  # ── done / undo_done ────────────────────────────────────────────────────────

  context "done" do
    it "dismisses the feed item, re-renders the list, and includes an Undo toast" do
      person, contact, thread, msg = make_person(name: "Sofia", email: "sofia@x.example", owe: true)
      item = make_feed_item(user: user, person: person, contact: contact,
                            thread: thread, msg: msg, kind: "reply_reminder")
      refresh_standings!
      link_feed_item!(person, item, msg) # patch standing since still_valid? is hard to satisfy in unit tests

      expect {
        post people_action_path(person.id, :done),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change { item.reload.dismissed_at }.from(nil)

      expect(response).to have_http_status(:ok)
      # Re-renders the counterpart list.
      # `update` keeps the people_results frame in place so the next action, the
      # search and the pagination still have their target.
      expect(response.body).to include('<turbo-stream action="update" target="people_results"')
      # Undo toast pointing to undo_done.
      expect(response.body).to include("undo_done")
    end

    it "stamps follow_up_dismissed_at for a follow_up item" do
      person, contact, thread, msg = make_person(name: "Sofia", email: "sofia@x.example", owe: true)
      item = make_feed_item(user: user, person: person, contact: contact,
                            thread: thread, msg: msg, kind: "follow_up")
      refresh_standings!
      link_feed_item!(person, item, msg)

      post people_action_path(person.id, :done),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      expect(thread.reload.follow_up_dismissed_at).not_to be_nil
      expect(item.reload.dismissed_at).not_to be_nil
    end

    it "404s when the standing row has no feed item of the right kind" do
      person, = make_person(name: "Sofia", email: "sofia@x.example")
      # No feed item; standing has no feed_item_id.
      refresh_standings!

      post people_action_path(person.id, :done),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:not_found)
    end
  end

  context "undo_done" do
    it "reactivates the feed item" do
      person, contact, thread, msg = make_person(name: "Sofia", email: "sofia@x.example", owe: true)
      item = make_feed_item(user: user, person: person, contact: contact,
                            thread: thread, msg: msg, kind: "reply_reminder")
      # Refresh BEFORE dismiss so the standing captures feed_item_id; the controller
      # looks up any feed item (not just active), so it can reactivate a dismissed one.
      refresh_standings!
      link_feed_item!(person, item, msg)
      item.dismiss!

      expect {
        post people_action_path(person.id, :undo_done),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change { item.reload.dismissed_at }.to(nil)

      expect(response).to have_http_status(:ok)
    end
  end

  # ── snooze / unsnooze ────────────────────────────────────────────────────────

  context "snooze" do
    it "runs the snooze tool and the row leaves the lane" do
      person, _contact, _thread, msg = make_person(name: "Rui", email: "rui@x.example", owe: true)
      refresh_standings!
      link_message!(person, msg)

      # Snooze the thread via the people action.
      allow(Tools::Snooze).to receive(:call).and_return(msg.email_thread.tap { |t| t.update_columns(snoozed_until: 1.day.from_now) })

      post people_action_path(person.id, :snooze),
           params: { until: "tomorrow" },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      # `update` keeps the people_results frame in place so the next action, the
      # search and the pagination still have their target.
      expect(response.body).to include('<turbo-stream action="update" target="people_results"')
    end
  end

  context "unsnooze" do
    it "runs the unsnooze tool" do
      person, _contact, _thread, msg = make_person(name: "Rui", email: "rui@x.example", owe: true)
      refresh_standings!
      link_message!(person, msg)

      allow(Tools::Unsnooze).to receive(:call).and_return(msg.email_thread)

      post people_action_path(person.id, :unsnooze),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
    end
  end

  # ── star / unstar ──────────────────────────────────────────────────────────

  context "star" do
    it "sets starred_at on the busiest contact and re-renders" do
      person, contact, = make_person(name: "Ana", email: "ana@x.example")
      refresh_standings!

      expect {
        post people_action_path(person.id, :star),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change { contact.reload.starred_at }.from(nil)

      expect(response).to have_http_status(:ok)
      # `update` keeps the people_results frame in place so the next action, the
      # search and the pagination still have their target.
      expect(response.body).to include('<turbo-stream action="update" target="people_results"')
    end
  end

  context "unstar" do
    it "clears starred_at on the busiest contact" do
      person, contact, = make_person(name: "Ana", email: "ana@x.example")
      contact.update_columns(starred_at: Time.current)
      refresh_standings!

      expect {
        post people_action_path(person.id, :unstar),
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change { contact.reload.starred_at }.to(nil)

      expect(response).to have_http_status(:ok)
    end
  end

  # ── archive / unarchive ───────────────────────────────────────────────────

  context "archive" do
    it "archives the thread via EmailActions and re-renders" do
      person, _contact, thread, msg = make_person(name: "Miguel", email: "miguel@x.example", owe: true)
      refresh_standings!
      link_message!(person, msg)

      allow(Tools::Archive).to receive(:call).and_return(true)

      post people_action_path(person.id, :archive),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
      # `update` keeps the people_results frame in place so the next action, the
      # search and the pagination still have their target.
      expect(response.body).to include('<turbo-stream action="update" target="people_results"')
    end
  end

  context "unarchive" do
    it "unarchives the thread" do
      person, _contact, _thread, msg = make_person(name: "Miguel", email: "miguel@x.example", owe: true)
      refresh_standings!
      link_message!(person, msg)

      allow(Tools::Unarchive).to receive(:call).and_return(true)

      post people_action_path(person.id, :unarchive),
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:ok)
    end
  end

  # ── failure → 422 ─────────────────────────────────────────────────────────

  it "returns 422 with a notice on action failure" do
    person, _contact, _thread, msg = make_person(name: "Rita", email: "rita@x.example", owe: true)
    refresh_standings!
    link_message!(person, msg) # ensure @message is found so the action reaches Tools::Archive

    allow(Tools::Archive).to receive(:call).and_return(false)

    post people_action_path(person.id, :archive),
         headers: { "Accept" => "text/vnd.turbo-stream.html" }

    expect(response).to have_http_status(:unprocessable_content)
  end

  # ── can_reply flag ────────────────────────────────────────────────────────

  it "sets can_reply false for a non-sendable account" do
    grant_access(can_send: false)  # second grant without send permission
    person, = make_person(name: "Tiago", email: "tiago@x.example")
    refresh_standings!

    row = PeopleStanding.for_user(user).find_by(counterpart: person)
    expect(row.data["can_reply"]).to be_falsy
  end
end
