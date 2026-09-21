# frozen_string_literal: true

require "rails_helper"

RSpec.describe "GET /api/app/people/:id", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  def grant_access
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
  end

  def make_person_with_thread
    person  = create(:person, workspace: workspace, name: "Sara Monteiro")
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                               name: "Sara Monteiro", email: "sara@example.com",
                               sender_kind: :person, sender_kind_source: "heuristic")
    thread  = create(:email_thread, email_account: account, subject: "Project update")
    msg     = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                                     from_address: "sara@example.com", subject: "Project update",
                                     received_at: 1.day.ago)
    contact.update_columns(email_count: 1, last_email_at: 1.day.ago)
    [ person, thread, msg ]
  end

  before { grant_access }

  let(:headers) { api_app_headers(user) }

  describe "unauthenticated" do
    it "returns 401" do
      get "/api/app/people/1"
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "authenticated" do
    it "returns 200 with person data" do
      person, thread, _msg = make_person_with_thread

      get "/api/app/people/#{person.id}", headers: headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["id"]).to eq(person.id)
      expect(body["name"]).to eq("Sara Monteiro")
      expect(body["threads"]).to be_an(Array)
      expect(body["threads_meta"]).to be_a(Hash)
    end

    it "returns threads with the first thread's messages loaded" do
      person, thread, msg = make_person_with_thread

      get "/api/app/people/#{person.id}", headers: headers

      first_thread = response.parsed_body.dig("data", "threads", 0)
      expect(first_thread["id"]).to eq(thread.id)
      expect(first_thread["loaded"]).to be true
      expect(first_thread["messages"]).to be_an(Array)
      expect(first_thread["messages"].first["id"]).to eq(msg.id)
    end

    it "404s for a person in another workspace" do
      other = create(:workspace)
      other_person = create(:person, workspace: other, name: "Ghost")

      get "/api/app/people/#{other_person.id}", headers: headers
      expect(response).to have_http_status(:not_found)
    end
  end
end
