# frozen_string_literal: true

require "rails_helper"

RSpec.describe "GET /api/app/today", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:headers)   { api_app_headers(user) }

  # Pin a stable date so calendar-based computations don't drift with CI clock.
  around { |example| travel_to(::Time.zone.parse("2026-09-21 09:00:00")) { example.run } }

  # ── Authentication ──────────────────────────────────────────────────────────

  describe "unauthenticated" do
    it "returns 401" do
      get "/api/app/today"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  # ── Happy path (no items) ───────────────────────────────────────────────────

  describe "authenticated, empty workspace" do
    it "returns 200 with the expected top-level shape" do
      get "/api/app/today", headers: headers
      expect(response).to have_http_status(:ok)

      body = response.parsed_body["data"]
      expect(body).to include("greeting", "needs_you", "coming_up", "handled")
    end

    it "returns empty needs_you and coming_up when there is nothing to act on" do
      get "/api/app/today", headers: headers
      body = response.parsed_body["data"]
      expect(body["needs_you"]).to eq([])
      expect(body["coming_up"]).to eq([])
    end

    it "returns zero counts in handled when there are no Scout events" do
      get "/api/app/today", headers: headers
      handled = response.parsed_body["data"]["handled"]
      expect(handled).to include("filed" => 0, "matched" => 0, "tucked" => 0, "added" => 0)
      expect(handled["since"]).to match(/\A\d{4}-\d{2}-\d{2}\z/)
    end

    it "returns a greeting with name, date, and brief" do
      get "/api/app/today", headers: headers
      greeting = response.parsed_body["data"]["greeting"]
      expect(greeting).to include("name", "date", "brief")
      expect(greeting["name"]).to be_a(String).and be_present
      expect(greeting["date"]).to eq("2026-09-21")
    end
  end

  # ── needs_you items from People standings ───────────────────────────────────

  describe "with a needing People standing" do
    let(:account) { create(:email_account, workspace: workspace) }

    before { create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true) }

    def make_needing_standing(name:, verb: "reply")
      person  = create(:person, workspace: workspace, name: name)
      contact = create(:contact, workspace: workspace, email_account: account,
                                 person: person, name: name, email: "#{name.downcase.gsub(" ", "")}@example.com",
                                 sender_kind: :person, sender_kind_source: "heuristic")
      thread  = create(:email_thread, email_account: account, subject: "Re: #{name}")
      create(:email_message, email_account: account, email_thread: thread, contact: contact,
                             from_address: contact.email, subject: "Re: #{name}", received_at: 2.days.ago)
      contact.update_columns(email_count: 1, last_email_at: 2.days.ago)
      People::Standings.refresh!(user)
      PeopleStanding.for_user(user).update_all(needs_you: true, verb: verb, standing_kind: "attention")
      person
    end

    it "includes the people item with the correct shape" do
      make_needing_standing(name: "Ana Costa", verb: "reply")

      get "/api/app/today", headers: headers
      needs_you = response.parsed_body["data"]["needs_you"]
      expect(needs_you).not_to be_empty

      item = needs_you.first
      expect(item).to include(
        "source"  => "people",
        "verb"    => "reply",
        "place"   => "Inbox",
        "title"   => "Ana Costa"
      )
      expect(item["id"]).to start_with("people_")
      expect(item["ref_id"]).to be_present
      expect(item["actions"]).to be_an(Array).and be_present
      expect(item["actions"].first).to include("kind", "label", "primary")
    end

    it "includes a snooze action alongside the primary action" do
      make_needing_standing(name: "Bruno Lima")

      get "/api/app/today", headers: headers
      item    = response.parsed_body["data"]["needs_you"].first
      actions = item["actions"]
      kinds   = actions.map { |a| a["kind"] }
      expect(kinds).to include("snooze")
    end

    it "archive action is self-describing with endpoint/method/body and undo" do
      person = make_needing_standing(name: "Carla Ferreira", verb: "decide")

      get "/api/app/today", headers: headers
      item = response.parsed_body["data"]["needs_you"].first
      archive_action = item["actions"].find { |a| a["kind"] == "archive" }

      expect(archive_action).to be_present
      expect(archive_action["endpoint"]).to eq("/api/app/people/#{person.id}/action")
      expect(archive_action["method"]).to eq("POST")
      expect(archive_action["body"]).to eq("kind" => "archive")
      expect(archive_action["undo"]).to include(
        "endpoint" => "/api/app/people/#{person.id}/action",
        "method"   => "POST"
      )
      expect(archive_action["undo"]["body"]).to eq("kind" => "unarchive")
    end

    it "snooze action is self-describing with endpoint/method/body and undo" do
      person = make_needing_standing(name: "Diogo Santos", verb: "reply")

      get "/api/app/today", headers: headers
      item          = response.parsed_body["data"]["needs_you"].first
      snooze_action = item["actions"].find { |a| a["kind"] == "snooze" }

      expect(snooze_action).to be_present
      expect(snooze_action["endpoint"]).to eq("/api/app/people/#{person.id}/action")
      expect(snooze_action["method"]).to eq("POST")
      expect(snooze_action["body"]).to eq("kind" => "snooze")
      expect(snooze_action["undo"]).to include(
        "endpoint" => "/api/app/people/#{person.id}/action",
        "method"   => "POST"
      )
      expect(snooze_action["undo"]["body"]).to eq("kind" => "unsnooze")
    end

    it "reply action omits endpoint (navigation-only)" do
      make_needing_standing(name: "Erica Lopes", verb: "reply")

      get "/api/app/today", headers: headers
      item         = response.parsed_body["data"]["needs_you"].first
      reply_action = item["actions"].find { |a| a["kind"] == "reply" }

      expect(reply_action).to be_present
      expect(reply_action.key?("endpoint")).to be false
    end
  end

  # ── Time items — asks ────────────────────────────────────────────────────────

  describe "with an undated ask (task)" do
    it "task done action is self-describing with endpoint/method" do
      task = Task.create!(workspace: workspace, title: "Review contract", status: :todo)

      get "/api/app/today", headers: headers
      needs_you = response.parsed_body["data"]["needs_you"]
      item = needs_you.find { |i| i["id"] == "time_task_#{task.id}" }

      expect(item).to be_present
      done_action = item["actions"].find { |a| a["kind"] == "done" }
      expect(done_action["endpoint"]).to eq("/api/app/asks/#{task.id}/done")
      expect(done_action["method"]).to eq("POST")

      snooze_action = item["actions"].find { |a| a["kind"] == "snooze" }
      expect(snooze_action["endpoint"]).to eq("/api/app/asks/#{task.id}/snooze")
      expect(snooze_action["method"]).to eq("POST")
    end
  end

  # ── Time items — deadlines (reminders) ───────────────────────────────────────

  describe "with an overdue reminder (deadline)" do
    it "confirm and dismiss actions are self-describing with endpoint/method" do
      reminder = create(:reminder, :overdue, workspace: workspace)

      get "/api/app/today", headers: headers
      needs_you = response.parsed_body["data"]["needs_you"]
      item = needs_you.find { |i| i["id"] == "time_deadline_#{reminder.id}" }

      expect(item).to be_present

      confirm_action = item["actions"].find { |a| a["kind"] == "confirm" }
      expect(confirm_action["endpoint"]).to eq("/api/app/reminders/#{reminder.id}/confirm")
      expect(confirm_action["method"]).to eq("POST")

      dismiss_action = item["actions"].find { |a| a["kind"] == "dismiss" }
      expect(dismiss_action["endpoint"]).to eq("/api/app/reminders/#{reminder.id}")
      expect(dismiss_action["method"]).to eq("DELETE")
    end
  end

  # ── Cross-workspace isolation ────────────────────────────────────────────────

  describe "cross-workspace isolation" do
    let(:other_workspace) { create(:workspace) }
    let(:other_account)   { create(:email_account, workspace: other_workspace) }
    let(:other_user)      { create(:user, workspace: other_workspace) }

    it "does not include another workspace's standing rows" do
      # Build a needing standing for other_user in other_workspace.
      create(:email_account_user, user: other_user, email_account: other_account,
             can_read: true, can_send: true)
      person  = create(:person, workspace: other_workspace, name: "Ghost Person")
      contact = create(:contact, workspace: other_workspace, email_account: other_account,
                                 person: person, name: "Ghost Person",
                                 email: "ghost@other.example", sender_kind: :person,
                                 sender_kind_source: "heuristic")
      thread  = create(:email_thread, email_account: other_account, subject: "Ghost thread")
      create(:email_message, email_account: other_account, email_thread: thread, contact: contact,
                             from_address: contact.email, received_at: 1.day.ago)
      contact.update_columns(email_count: 1, last_email_at: 1.day.ago)
      People::Standings.refresh!(other_user)
      PeopleStanding.for_user(other_user).update_all(needs_you: true, verb: "reply", standing_kind: "attention")

      # Our user (different workspace) should see nothing from other_workspace.
      get "/api/app/today", headers: headers
      needs_you = response.parsed_body["data"]["needs_you"]
      titles    = needs_you.map { |i| i["title"] }
      expect(titles).not_to include("Ghost Person")
    end
  end

  # ── Money items absent when not entitled ────────────────────────────────────

  describe "money gate" do
    it "omits money items when accounting is disabled" do
      with_env("ENABLE_ACCOUNTING" => "0") do
        get "/api/app/today", headers: headers
        needs_you = response.parsed_body["data"]["needs_you"]
        money_items = needs_you.select { |i| i["source"] == "money" }
        expect(money_items).to eq([])
      end
    end

    it "omits money items when accounting is enabled but the workspace is not entitled" do
      with_env("ENABLE_ACCOUNTING" => "1") do
        # Default workspace has no :accounting entitlement, so money items are skipped.
        get "/api/app/today", headers: headers
        needs_you   = response.parsed_body["data"]["needs_you"]
        money_items = needs_you.select { |i| i["source"] == "money" }
        expect(money_items).to eq([])
      end
    end
  end

  # ── handled counts ──────────────────────────────────────────────────────────

  describe "handled counts from Scout system events" do
    let(:event_workspace) { workspace }

    def create_system_event(name, payload = {})
      create(:event, workspace: event_workspace, name: name, payload: payload,
             actor_id: nil, occurred_at: 1.hour.ago)
    end

    it "counts document.processed as filed" do
      create_system_event("document.processed")
      create_system_event("document.processed")

      get "/api/app/today", headers: headers
      expect(response.parsed_body["data"]["handled"]["filed"]).to eq(2)
    end

    it "counts email.archived as matched" do
      create_system_event("email.archived")
      create_system_event("email.bulk_archived", { "count" => 5 })

      get "/api/app/today", headers: headers
      expect(response.parsed_body["data"]["handled"]["matched"]).to eq(6)
    end

    it "counts email.tagged as tucked" do
      create_system_event("email.tagged")

      get "/api/app/today", headers: headers
      expect(response.parsed_body["data"]["handled"]["tucked"]).to eq(1)
    end

    it "counts task.created + reminder.created as added" do
      create_system_event("task.created")
      create_system_event("reminder.created")

      get "/api/app/today", headers: headers
      expect(response.parsed_body["data"]["handled"]["added"]).to eq(2)
    end
  end
end
