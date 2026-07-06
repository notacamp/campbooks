require "rails_helper"

RSpec.describe Feed::Sources::TagSuggestion do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:contact)   { create(:contact, workspace: workspace) }
  let(:tag)       { create(:tag, workspace: workspace, name: "Invoices") }

  before { create(:email_account_user, user: user, email_account: account, can_read: true) }

  def tagged_email(tag_name: "invoices")
    create(:email_message, email_account: account, contact_id: contact.id, from_address: "billing@acme.com",
           ai_todo_dismissed: false, skimmed_at: nil,
           ai_suggested_actions: [ { "tool" => "add_tag", "args" => { "tag_name" => tag_name } } ])
  end

  def record(label, tag_name: "invoices", n: 3)
    n.times do
      LearningDecision.create!(domain: "tag_suggestion", user: user, workspace_id: workspace.id,
                               label: label, contact_id: contact.id, sender_domain: "acme.com",
                               signals: { "tag_name" => tag_name })
    end
  end

  def candidates = described_class.new(user).candidates

  # Ensure the tag exists in the workspace for auto-apply to succeed.
  before { tag }

  it "surfaces a tag suggestion" do
    email = tagged_email
    expect(candidates.map { |c| c[:subject].id }).to include(email.id)
  end

  it "auto-applies the tag at generation time" do
    email = tagged_email
    candidates
    expect(email.reload.tags.map { |t| t.name.downcase }).to include("invoices")
  end

  it "stamps data[applied] = true when the tag is successfully applied" do
    email = tagged_email
    card = candidates.find { |c| c[:subject].id == email.id }
    expect(card[:data]["applied"]).to be true
  end

  it "is idempotent — running candidates twice does not create duplicate tags" do
    email = tagged_email
    2.times { described_class.new(user).candidates }
    expect(email.reload.email_message_tags.count).to eq(1)
  end

  it "stamps data[applied] = true even when the tag was already applied" do
    email = tagged_email
    # Pre-apply the tag (simulates a prior generation run).
    email.email_message_tags.create!(tag: tag)
    card = candidates.find { |c| c[:subject].id == email.id }
    expect(card[:data]["applied"]).to be true
  end

  it "drops a suggestion the user keeps rejecting for this sender + tag" do
    email = tagged_email
    record("rejected")
    expect(candidates.map { |c| c[:subject].id }).not_to include(email.id)
  end

  it "still surfaces a different tag from the same sender" do
    other_tag = create(:tag, workspace: workspace, name: "Receipts")
    email = tagged_email(tag_name: "receipts")
    record("rejected", tag_name: "invoices")
    result = candidates.find { |c| c[:subject].id == email.id }
    expect(result).to be_present
  end

  it "boosts the score of an always-accepted tag" do
    email = tagged_email
    record("accepted")
    card = candidates.find { |c| c[:subject].id == email.id }
    expect(card[:score]).to eq(25)
  end

  describe "#still_valid?" do
    subject(:source) { described_class.new(user) }

    def item_for(email, tag_name: "invoices", applied: true)
      instance_double(FeedItem, data: { "tag_name" => tag_name, "applied" => applied })
    end

    it "is valid for a notice card while the tag is still applied" do
      email = tagged_email
      email.email_message_tags.create!(tag: tag)
      email.reload
      expect(source.still_valid?(item_for(email), email)).to be true
    end

    it "is invalid for a notice card once the tag is removed" do
      email = tagged_email
      # Tag is not on the email (was removed after the notice was generated).
      email.reload
      expect(source.still_valid?(item_for(email), email)).to be false
    end

    it "is valid for a legacy ask card while the tag is not yet applied" do
      email = tagged_email
      email.reload
      expect(source.still_valid?(item_for(email, applied: false), email)).to be true
    end

    it "is invalid for a legacy ask card once the tag has been applied" do
      email = tagged_email
      email.email_message_tags.create!(tag: tag)
      email.reload
      expect(source.still_valid?(item_for(email, applied: false), email)).to be false
    end

    it "is invalid for a nil subject" do
      expect(source.still_valid?(item_for(nil), nil)).to be false
    end
  end
end
