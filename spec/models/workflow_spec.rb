require "rails_helper"

RSpec.describe Workflow, type: :model do
  it "validates trigger_type inclusion" do
    workflow = build(:workflow, trigger_type: "nonsense")
    expect(workflow).not_to be_valid
    expect(workflow.errors[:trigger_type]).to be_present
  end

  describe "webhook token" do
    it "mints a token automatically when the trigger is a webhook" do
      workflow = create(:workflow, :webhook)
      expect(workflow.webhook_token).to be_present
      expect(workflow).to be_webhook
    end

    it "does not mint a token for email triggers" do
      workflow = create(:workflow, trigger_type: "email_received")
      expect(workflow.webhook_token).to be_nil
    end

    it "mints a token when an existing workflow switches to webhook" do
      workflow = create(:workflow, trigger_type: "email_received")
      workflow.update!(trigger_type: "webhook")
      expect(workflow.webhook_token).to be_present
    end

    it "rotates the token on regenerate" do
      workflow = create(:workflow, :webhook)
      original = workflow.webhook_token
      workflow.regenerate_webhook_token!
      expect(workflow.webhook_token).to be_present
      expect(workflow.webhook_token).not_to eq(original)
    end
  end
end
