require "rails_helper"

RSpec.describe Feed::Sources::TagSuggestion do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:contact)   { create(:contact, workspace: workspace) }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  def tagged_email(tag: "invoices")
    create(:email_message, email_account: account, contact_id: contact.id, from_address: "billing@acme.com",
           ai_todo_dismissed: false, skimmed_at: nil,
           ai_suggested_actions: [ { "tool" => "add_tag", "args" => { "tag_name" => tag } } ])
  end

  def record(label, tag: "invoices", n: 3)
    n.times do
      LearningDecision.create!(domain: "tag_suggestion", user: user, workspace_id: workspace.id,
                               label: label, contact_id: contact.id, sender_domain: "acme.com",
                               signals: { "tag_name" => tag })
    end
  end

  def candidates = described_class.new(user).candidates

  it "surfaces a tag suggestion by default" do
    email = tagged_email

    expect(candidates.map { |c| c[:subject].id }).to include(email.id)
  end

  it "drops a suggestion the user keeps rejecting for this sender + tag" do
    email = tagged_email
    record("rejected")

    expect(candidates.map { |c| c[:subject].id }).not_to include(email.id)
  end

  it "still surfaces a different tag from the same sender" do
    email = tagged_email(tag: "receipts")
    record("rejected", tag: "invoices")

    expect(candidates.map { |c| c[:subject].id }).to include(email.id)
  end

  it "boosts the score of an always-accepted tag" do
    email = tagged_email
    record("accepted")

    card = candidates.find { |c| c[:subject].id == email.id }
    expect(card[:score]).to eq(10)
  end
end
