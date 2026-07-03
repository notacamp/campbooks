require "test_helper"

class LearningDecisionTest < ActiveSupport::TestCase
  test "domain and label are required" do
    decision = LearningDecision.new
    assert_not decision.valid?
    assert decision.errors[:domain].any?, "expected domain to be required"
    assert decision.errors[:label].any?, "expected label to be required"
  end

  test "persists with a domain, label, workspace and user" do
    workspace = Workspace.create!(name: "Learning WS")
    user = workspace.users.create!(name: "Learner", email_address: "learner@example.com", password: "changeme123")

    decision = LearningDecision.create!(
      domain: "tag_suggestion", label: "rejected", workspace: workspace, user: user,
      sender_domain: "acme.com", signals: { "tag_name" => "invoices" }
    )

    assert decision.persisted?
    assert_equal "invoices", decision.signals["tag_name"]
  end
end
