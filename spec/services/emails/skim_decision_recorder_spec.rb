require "rails_helper"

RSpec.describe Emails::SkimDecisionRecorder do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }
  let(:account) { create(:email_account, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  def msg(**attrs)
    create(:email_message, **{ email_account: account }.merge(attrs))
  end

  it "records one decision per cluster, keyed on the representative (newest) email" do
    contact = create(:contact, workspace: workspace, email_account: account)
    older = msg(from_address: "no-reply@github.com", contact: contact, received_at: 2.hours.ago)
    newer = msg(from_address: "no-reply@github.com", contact: contact, received_at: 1.hour.ago)

    expect do
      described_class.record(user, [ older.id, newer.id ], action: "archive")
    end.to change(SkimDecision, :count).by(1)

    decision = SkimDecision.last
    expect(decision).to have_attributes(
      action: "archive",
      user_id: user.id,
      workspace_id: workspace.id,
      contact_id: contact.id,
      sender_domain: "github.com",
      email_message_id: newer.id
    )
  end

  it "derives the category from the representative email" do
    email = msg(from_address: "news@shop.com", subject: "50% off everything")

    described_class.record(user, [ email.id ], action: "keep")

    expect(SkimDecision.last.category).to eq("promotions")
  end

  it "ignores actions that aren't learnable triage verbs" do
    email = msg
    expect { described_class.record(user, [ email.id ], action: "block_sender") }
      .not_to change(SkimDecision, :count)
  end

  it "never records against mail the user cannot read" do
    foreign = create(:email_message, email_account: create(:email_account, workspace: create(:workspace)))
    expect { described_class.record(user, [ foreign.id ], action: "archive") }
      .not_to change(SkimDecision, :count)
  end

  it "is a no-op when there are no email ids" do
    expect { described_class.record(user, [], action: "keep") }.not_to change(SkimDecision, :count)
  end
end
