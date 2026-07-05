require "rails_helper"

RSpec.describe LearningDecision do
  it "domain and label are required" do
    decision = LearningDecision.new
    expect(decision).not_to be_valid
    expect(decision.errors[:domain]).to be_any, "expected domain to be required"
    expect(decision.errors[:label]).to be_any, "expected label to be required"
  end

  it "persists with a domain, label, workspace and user" do
    workspace = Workspace.create!(name: "Learning WS")
    user = workspace.users.create!(name: "Learner", email_address: "learner@example.com", password: "changeme123")

    decision = LearningDecision.create!(
      domain: "tag_suggestion", label: "rejected", workspace: workspace, user: user,
      sender_domain: "acme.com", signals: { "tag_name" => "invoices" }
    )

    expect(decision).to be_persisted
    expect(decision.signals["tag_name"]).to eq("invoices")
  end
end
