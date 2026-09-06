# frozen_string_literal: true

require "rails_helper"

RSpec.describe "People details attention verdicts", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    sign_in(user)
  end

  let(:person) { create(:person, workspace: workspace, name: "Sofia Martins") }
  let!(:contact) do
    create(:contact, workspace: workspace, email_account: account, person: person,
           email: "sofia@brightloop.example", sender_kind: :person, email_count: 2)
  end

  describe "POST /people/:id/details/attention" do
    it "records an important verdict and re-renders the frame" do
      expect {
        post attention_people_details_path(person), params: { verdict: "important" },
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change { LearningDecision.where(domain: "attention", user: user).count }.by(1)

      expect(response).to have_http_status(:ok)
      expect(LearningDecision.where(domain: "attention", user: user).last.label).to eq("important")
    end

    it "records an unimportant verdict" do
      expect {
        post attention_people_details_path(person), params: { verdict: "unimportant" },
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change { LearningDecision.where(domain: "attention", user: user).count }.by(1)

      expect(response).to have_http_status(:ok)
      expect(LearningDecision.where(domain: "attention", user: user).last.label).to eq("unimportant")
    end

    it "forgets a verdict" do
      LearningDecision.create!(domain: "attention", user: user, workspace_id: workspace.id,
                               label: "important", contact_id: contact.id, signals: {})

      expect {
        post attention_people_details_path(person), params: { verdict: "forget" },
             headers: { "Accept" => "text/vnd.turbo-stream.html" }
      }.to change { LearningDecision.where(domain: "attention", user: user).count }.by(-1)

      expect(response).to have_http_status(:ok)
    end

    it "returns 422 for an invalid verdict" do
      post attention_people_details_path(person), params: { verdict: "invalid" },
           headers: { "Accept" => "text/vnd.turbo-stream.html" }

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end
end
