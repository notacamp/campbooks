# frozen_string_literal: true

require "rails_helper"

RSpec.describe "POST /api/app/people/:id/action", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  def grant_access
    create(:email_account_user, user: user, email_account: account, can_read: true, can_send: true)
  end

  def make_person_with_message
    person  = create(:person, workspace: workspace, name: "Tiago Rocha")
    contact = create(:contact, workspace: workspace, email_account: account, person: person,
                               name: "Tiago Rocha", email: "tiago@example.com",
                               sender_kind: :person, sender_kind_source: "heuristic")
    thread  = create(:email_thread, email_account: account, subject: "Invoice")
    message = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                                     from_address: "tiago@example.com", subject: "Invoice",
                                     received_at: 3.days.ago)
    contact.update_columns(email_count: 1, last_email_at: 3.days.ago)
    People::Standings.refresh!(user)
    [ person, message ]
  end

  before { grant_access }

  let(:headers) { api_app_headers(user) }

  describe "unauthenticated" do
    it "returns 401" do
      post "/api/app/people/1/action", params: { kind: "archive" }
      expect(response).to have_http_status(:unauthorized)
    end
  end

  describe "archive (P1 pilot action)" do
    it "archives the email and returns the refreshed row + undo_kind" do
      person, message = make_person_with_message
      # Ensure the standing row has the message id
      PeopleStanding.for_user(user).update_all(email_message_id: message.id)

      # In test environments Tools::Archive hits a stub mail client;
      # mirror the web People::ActionsController spec's approach.
      allow(Tools::Archive).to receive(:call).and_return(true)

      post "/api/app/people/#{person.id}/action",
           params: { kind: "archive" }, headers: headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["undo_kind"]).to eq("unarchive")
      expect(body["message"]).to be_present
    end

    it "returns 422 when the row has no associated message" do
      person, = make_person_with_message
      PeopleStanding.for_user(user).update_all(email_message_id: nil)

      post "/api/app/people/#{person.id}/action",
           params: { kind: "archive" }, headers: headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("action_failed")
    end
  end

  describe "unsupported kind" do
    it "returns 422 for an unknown kind" do
      person, = make_person_with_message

      post "/api/app/people/#{person.id}/action",
           params: { kind: "fly_to_moon" }, headers: headers

      expect(response).to have_http_status(:unprocessable_entity)
      expect(response.parsed_body.dig("error", "code")).to eq("unsupported_action")
    end
  end

  describe "cross-workspace isolation" do
    it "returns 404 for a person in another workspace" do
      other_workspace = create(:workspace)
      other_user = create(:user, workspace: other_workspace)
      other_person = create(:person, workspace: other_workspace, name: "Isolation Person")

      post "/api/app/people/#{other_person.id}/action",
           params: { kind: "archive" }, headers: headers

      expect(response).to have_http_status(:not_found)
    end
  end

  describe "star action" do
    it "stars the contact and returns undo_kind: unstar" do
      person, = make_person_with_message

      post "/api/app/people/#{person.id}/action",
           params: { kind: "star" }, headers: headers

      expect(response).to have_http_status(:ok)
      body = response.parsed_body["data"]
      expect(body["undo_kind"]).to eq("unstar")
    end
  end

  describe "paid action" do
    it "settles the person's late invoice and dismisses the card" do
      person, = make_person_with_message
      document = create(:document, workspace: workspace)
      item = FeedItem.create!(user: user, workspace: workspace, subject: document, kind: "late_payable",
                              dedupe_key: "late_payable:#{document.id}", sort_at: Time.current, score: 60)
      PeopleStanding.for_user(user).update_all(feed_item_id: item.id)

      post "/api/app/people/#{person.id}/action",
           params: { kind: "paid" }, headers: headers

      expect(response).to have_http_status(:ok)
      expect(response.parsed_body.dig("data", "message")).to be_present
      expect(document.reload.settled?).to be(true)
      expect(item.reload.dismissed?).to be(true)
    end

    it "422s when the person has no late invoice" do
      person, = make_person_with_message

      post "/api/app/people/#{person.id}/action",
           params: { kind: "paid" }, headers: headers

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end
end
