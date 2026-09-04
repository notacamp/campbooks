# frozen_string_literal: true

require "rails_helper"

RSpec.describe "People::Details", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    sign_in(user)
  end

  let(:person) { create(:person, workspace: workspace, name: "Test Person") }
  let!(:contact) do
    create(:contact, workspace: workspace, email_account: account,
           person: person, email: "test@example.com", sender_kind: :person, email_count: 3)
  end

  describe "GET /people/:id/details" do
    it "renders 200 with the Details frame" do
      get people_details_path(person),
          headers: { "Accept" => "text/html", "Turbo-Frame" => "people_details" }
      expect(response).to have_http_status(:ok)
    end

    it "includes the person's name" do
      get people_details_path(person)
      expect(response.body).to include("Test Person")
    end

    it "returns 404 for a person in another workspace" do
      other_ws   = create(:workspace)
      other_user = create(:user, workspace: other_ws)
      other_person = create(:person, workspace: other_ws, name: "Other")
      get people_details_path(other_person)
      expect(response).to have_http_status(:not_found)
    end
  end

  describe "PATCH /people/:id/details/rename" do
    it "renames the person and re-renders the frame" do
      patch rename_people_details_path(person),
            params: { name: "Renamed Person" },
            headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:ok)
      expect(person.reload.name).to eq("Renamed Person")
    end
  end

  describe "PATCH /people/:id/details/relationship" do
    it "updates the relationship type" do
      patch relationship_people_details_path(person),
            params: { relationship_type: "client" },
            headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:ok)
      expect(person.reload.relationship_type).to eq("client")
    end
  end

  describe "PATCH /people/:id/details/kind" do
    it "marks the contact as service" do
      patch kind_people_details_path(person),
            params: { kind: "service" },
            headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:ok)
      expect(contact.reload.sender_kind).to eq("service")
    end

    it "returns 422 for an invalid kind" do
      patch kind_people_details_path(person),
            params: { kind: "invalid" },
            headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:unprocessable_entity)
    end
  end

  describe "PATCH /people/:id/details/state" do
    it "stars the primary contact" do
      patch state_people_details_path(person),
            params: { state: "star" },
            headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:ok)
      expect(contact.reload.starred?).to be true
    end

    it "blocks the contact (undoable)" do
      patch state_people_details_path(person),
            params: { state: "block" },
            headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:ok)
      expect(contact.reload.blocked?).to be true
      # Response should include an undo stream; ActionToast POSTs state=unblock.
      expect(response.body).to include("unblock")
    end

    it "unblocks the contact" do
      contact.update!(list_status: :blocked)
      patch state_people_details_path(person),
            params: { state: "unblock" },
            headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:ok)
      expect(contact.reload.neutral?).to be true
    end

    it "POST undo after star unstars via the state endpoint" do
      contact.star!
      post state_people_details_path(person),
           params: { state: "unstar" },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:ok)
      expect(contact.reload.starred?).to be false
    end
  end

  describe "cross-workspace 404" do
    it "returns 404 for a person not owned by the workspace" do
      other_ws = create(:workspace)
      other_person = create(:person, workspace: other_ws, name: "Stranger")
      patch rename_people_details_path(other_person),
            params: { name: "Hack" },
            headers: { "Accept" => "text/vnd.turbo-stream.html" }
      expect(response).to have_http_status(:not_found)
    end
  end
end
