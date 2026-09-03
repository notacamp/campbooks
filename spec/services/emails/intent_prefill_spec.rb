require "rails_helper"

RSpec.describe Emails::IntentPrefill, type: :service do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace) }

  def prefill(intent: nil, to: nil)
    described_class.for(user: user, intent: intent, to: to)
  end

  def contact(name:, email:, **attrs)
    create(:contact, workspace: workspace, name: name, email: email, **attrs)
  end

  describe "recipient from an explicit ?to= address" do
    it "uses the address and marks it inferred" do
      result = prefill(to: "sofia@example.com")

      expect(result.to).to eq("sofia@example.com")
      expect(result.to_inferred?).to be(true)
    end

    it "prefers a matching contact's display name for the chip" do
      contact(name: "Sofia Martins", email: "sofia@example.com")

      expect(prefill(to: "sofia@example.com").to).to eq("Sofia Martins <sofia@example.com>")
    end

    it "ignores a value that is not an email address" do
      expect(prefill(to: "not-an-email").to).to eq("")
    end
  end

  describe "recipient matched from the intent text" do
    it "matches a contact by first name" do
      contact(name: "Sofia Martins", email: "sofia@example.com")

      result = prefill(intent: "write to Sofia about the Q3 deck")

      expect(result.to).to eq("Sofia Martins <sofia@example.com>")
      expect(result.to_inferred?).to be(true)
      expect(result.contact&.email).to eq("sofia@example.com")
    end

    it "matches a contact by last name" do
      contact(name: "Miguel Costa", email: "miguel@example.com")

      expect(prefill(intent: "ping Costa for the signed contract").to)
        .to eq("Miguel Costa <miguel@example.com>")
    end

    it "does not match a bare email when the name is not a contact" do
      contact(name: "Sofia Martins", email: "sofia@example.com")

      expect(prefill(intent: "remind the team about lunch").to).to eq("")
    end

    it "prefers a starred contact when two names collide" do
      contact(name: "Sofia Martins", email: "martins@example.com")
      starred = contact(name: "Sofia Reis", email: "reis@example.com", starred_at: 1.day.ago)

      expect(prefill(intent: "message Sofia the update").contact).to eq(starred)
    end

    it "prefers the more recently contacted match when neither is starred" do
      contact(name: "Sofia Martins", email: "martins@example.com", last_email_at: 30.days.ago)
      recent = contact(name: "Sofia Reis", email: "reis@example.com", last_email_at: 1.day.ago)

      expect(prefill(intent: "message Sofia the update").contact).to eq(recent)
    end

    it "returns no recipient when the match is genuinely ambiguous" do
      contact(name: "Sofia Martins", email: "martins@example.com")
      contact(name: "Sofia Reis", email: "reis@example.com")

      result = prefill(intent: "message Sofia the update")

      expect(result.to).to eq("")
      expect(result.to_inferred?).to be(false)
      expect(result.contact).to be_nil
    end

    it "scopes matching to the user's own workspace" do
      other = create(:workspace)
      create(:contact, workspace: other, name: "Sofia Martins", email: "sofia@example.com")

      expect(prefill(intent: "write to Sofia about the deck").to).to eq("")
    end
  end

  describe "subject from the intent's trailing about clause" do
    it "extracts and capitalizes the clause" do
      contact(name: "Sofia Martins", email: "sofia@example.com")

      result = prefill(intent: "write to Sofia about the Q3 kickoff deck")

      expect(result.subject).to eq("The Q3 kickoff deck")
      expect(result.subject_inferred?).to be(true)
    end

    it "strips trailing punctuation from the clause" do
      expect(prefill(intent: "email finance about the overdue invoice?").subject)
        .to eq("The overdue invoice")
    end
  end

  describe "subject from the latest thread when the intent asks to reply" do
    let(:account) { create(:email_account, workspace: workspace, email_address: "me@example.com") }

    before { create(:email_account_user, user: user, email_account: account, can_read: true) }

    it "uses Re: <latest subject> for a reply-cue intent" do
      person = contact(name: "Sofia Martins", email: "sofia@example.com")
      create(:email_message, email_account: account, contact: person,
                             subject: "Q3 kickoff deck", received_at: 2.days.ago)
      create(:email_message, email_account: account, contact: person,
                             subject: "Pricing questions", received_at: 1.hour.ago)

      result = prefill(intent: "reply to Sofia")

      expect(result.subject).to eq("Re: Pricing questions")
      expect(result.subject_inferred?).to be(true)
      expect(result.reply_source&.subject).to eq("Pricing questions")
    end

    it "does not prefix Re: twice" do
      person = contact(name: "Sofia Martins", email: "sofia@example.com")
      create(:email_message, email_account: account, contact: person,
                             subject: "Re: Pricing questions", received_at: 1.hour.ago)

      expect(prefill(intent: "follow up with Sofia").subject).to eq("Re: Pricing questions")
    end

    it "does not infer a thread subject without a reply cue" do
      person = contact(name: "Sofia Martins", email: "sofia@example.com")
      create(:email_message, email_account: account, contact: person,
                             subject: "Pricing questions", received_at: 1.hour.ago)

      result = prefill(intent: "message Sofia the update")

      expect(result.subject).to eq("")
      expect(result.reply_source).to be_nil
    end

    it "ignores a thread on an account the user cannot read" do
      person = contact(name: "Sofia Martins", email: "sofia@example.com")
      hidden = create(:email_account, workspace: workspace)
      create(:email_message, email_account: hidden, contact: person,
                             subject: "Private thread", received_at: 1.hour.ago)

      expect(prefill(intent: "reply to Sofia").subject).to eq("")
    end
  end

  describe "nothing to infer" do
    it "returns an empty result" do
      result = prefill(intent: "just some notes to myself")

      expect(result.any?).to be(false)
      expect(result.to).to eq("")
      expect(result.subject).to eq("")
    end
  end
end
