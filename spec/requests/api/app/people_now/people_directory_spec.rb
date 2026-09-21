# frozen_string_literal: true

require "rails_helper"

RSpec.describe "GET /api/app/people", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  def grant_access
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
  end

  def make_person(name:, email:, inbound_at: 2.days.ago, emails: 1)
    person  = create(:person, workspace: workspace, name: name)
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                               name: name, email: email, sender_kind: :person, sender_kind_source: "heuristic")
    thread  = create(:email_thread, email_account: account, subject: "Re: #{name}")
    create(:email_message, email_account: account, email_thread: thread, contact: contact,
                           from_address: email, subject: "Re: #{name}", received_at: inbound_at)
    contact.update_columns(email_count: emails, last_email_at: inbound_at)
    person
  end

  def refresh_standings!
    People::Standings.refresh!(user)
  end

  before { grant_access }

  describe "unauthenticated" do
    it "returns 401" do
      get "/api/app/people"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "authenticated" do
    let(:headers) { api_app_headers(user) }

    it "returns 200 with data array when standings exist" do
      make_person(name: "Ines Sousa", email: "ines@example.com")
      refresh_standings!

      get "/api/app/people", headers: headers
      expect(response).to have_http_status(:ok)
      body = response.parsed_body
      expect(body["data"]).to be_an(Array)
      expect(body["meta"]).to be_a(Hash)
    end

    # Regression: inside Api::App::People the bare `StandingsRefreshJob` resolved
    # to a nonexistent sibling constant and 500'd the whole Inbox. Only the stale
    # branch (rows older than 10 min) reaches it, which fresh test data never hit.
    it "does not 500 when standings are stale — enqueues the top-level refresh job" do
      make_person(name: "Ines Sousa", email: "ines@example.com")
      refresh_standings!
      PeopleStanding.for_user(user).update_all(refreshed_at: 11.minutes.ago)

      expect(::People::StandingsRefreshJob).to receive(:enqueue_for).with(user.id)

      get "/api/app/people", headers: headers
      expect(response).to have_http_status(:ok)
    end

    # Regression: the serializer referenced the wrong (sibling) StandCopy constant,
    # and a rescue swallowed the NameError, so stand_line was silently always nil
    # (the missing Inbox annotations). Prove it now calls the top-level ::People one.
    it "renders stand_line from ::People::StandCopy (not a swallowed nil)" do
      make_person(name: "Ines Sousa", email: "ines@example.com")
      refresh_standings!
      allow(::People::StandCopy).to receive(:line).and_return("Owes you a reply")

      get "/api/app/people", headers: headers
      row = response.parsed_body["data"].find { |r| r["name"] == "Ines Sousa" }
      expect(row).to be_present
      expect(row["stand_line"]).to eq("Owes you a reply")
    end

    it "returns empty data array when no contacts" do
      get "/api/app/people", headers: headers
      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"]).to eq([])
    end

    it "includes the expected row fields" do
      make_person(name: "Carlos Lima", email: "carlos@example.com")
      refresh_standings!

      get "/api/app/people", headers: headers
      row = response.parsed_body["data"].first
      expect(row).to include("id", "name", "counterpart_type", "needs_you", "score",
                              "last_activity_at", "stand_line", "verb", "wait_days")
      expect(row["name"]).to eq("Carlos Lima")
    end

    it "applies the ?q= search filter" do
      make_person(name: "Ana Costa",   email: "ana@example.com")
      make_person(name: "Bruno Ferreira", email: "bruno@example.com")
      refresh_standings!

      get "/api/app/people", params: { q: "Ana" }, headers: headers
      names = response.parsed_body["data"].map { |r| r["name"] }
      expect(names).to include("Ana Costa")
      expect(names).not_to include("Bruno Ferreira")
    end

    it "returns a different workspace's person as 404 isolation (cross-workspace)" do
      other_workspace = create(:workspace)
      other_user = create(:user, workspace: other_workspace)
      make_person(name: "Ghost Person", email: "ghost@other.example")

      # Ghost person's standings only exist for 'user', not other_user
      get "/api/app/people", headers: api_app_headers(other_user)
      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"]).to eq([])
    end

    it "supports the ?tab=needing param" do
      make_person(name: "Needs You", email: "needsyou@example.com")
      refresh_standings!
      # Force a needing row
      PeopleStanding.for_user(user).update_all(needs_you: true, verb: "reply", standing_kind: "attention")

      get "/api/app/people", params: { tab: "needing" }, headers: headers
      expect(response).to have_http_status(:ok)
      expect(response.parsed_body["data"]).to be_an(Array)
    end
  end
end
