require "rails_helper"

# The controller wiring: each Skim decision endpoint logs a LearningDecision
# (domain: "email_skim") so the learning loop (Emails::SkimActionMemory) has
# something to learn from. The signature derivation itself is covered in
# skim_decision_recorder_spec.
RSpec.describe "Skim decisions", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }
  let(:email) { create(:email_message, email_account: account, from_address: "no-reply@github.com") }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    sign_in(user)
  end

  it "logs an archive decision when a stack is archived" do
    expect do
      post "/skim/decide", params: { email_ids: [ email.id ] }, as: :json
    end.to change { LearningDecision.where(domain: "email_skim", label: "archive", user: user).count }.by(1)

    expect(response).to have_http_status(:ok)
  end

  it "logs a keep decision when a stack is kept" do
    expect do
      post "/skim/keep", params: { email_ids: [ email.id ] }, as: :json
    end.to change { LearningDecision.where(domain: "email_skim", label: "keep", user: user).count }.by(1)
  end

  it "logs a promote decision when a stack is pinned" do
    expect do
      post "/skim/promote", params: { email_ids: [ email.id ] }, as: :json
    end.to change { LearningDecision.where(domain: "email_skim", label: "promote", user: user).count }.by(1)
  end

  it "does not block the action when logging has nothing to record" do
    expect do
      post "/skim/keep", params: { email_ids: [] }, as: :json
    end.not_to change(LearningDecision, :count)
    expect(response).to have_http_status(:ok)
  end
end
