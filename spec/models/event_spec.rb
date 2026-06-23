require "rails_helper"

RSpec.describe Event, type: :model do
  describe "validations" do
    it "requires a name" do
      event = build(:event, name: nil)
      expect(event).not_to be_valid
      expect(event.errors[:name]).to be_present
    end
  end

  describe "occurred_at default" do
    it "defaults to now on create when unset" do
      event = create(:event, occurred_at: nil)
      expect(event.occurred_at).to be_within(5.seconds).of(Time.current)
    end

    it "keeps an explicit occurred_at" do
      ts = 3.days.ago
      expect(create(:event, occurred_at: ts).occurred_at).to be_within(1.second).of(ts)
    end
  end

  describe "associations" do
    it "links an optional polymorphic subject and actor" do
      workspace = create(:workspace)
      user = create(:user, workspace: workspace)
      contact = create(:contact, workspace: workspace)
      event = create(:event, workspace: workspace, subject: contact, actor: user)

      expect(event.subject).to eq(contact)
      expect(event.actor).to eq(user)
    end

    it "treats a nil actor as a system event" do
      expect(create(:event, actor: nil)).to be_system
    end

    it "chains via caused_by_event" do
      parent = create(:event)
      child = create(:event, workspace: parent.workspace, caused_by_event: parent)
      expect(child.caused_by_event).to eq(parent)
    end
  end

  describe "scopes" do
    it "orders recent first" do
      old = create(:event, occurred_at: 2.days.ago)
      fresh = create(:event, occurred_at: 1.minute.ago)
      expect(Event.recent.first).to eq(fresh)
      expect(Event.recent.last).to eq(old)
    end

    it "filters by name and subject" do
      workspace = create(:workspace)
      doc = create(:document, workspace: workspace)
      match = create(:event, workspace: workspace, name: "document.approved", subject: doc)
      create(:event, workspace: workspace, name: "document.rejected", subject: doc)

      expect(Event.named("document.approved")).to contain_exactly(match)
      expect(Event.for_subject(doc).count).to eq(2)
    end
  end

  describe ".accessible_to" do
    let(:workspace) { create(:workspace) }
    let(:user) { create(:user, workspace: workspace) }

    it "returns none for a nil user" do
      create(:event, workspace: workspace)
      expect(Event.accessible_to(nil)).to be_empty
    end

    it "hides events whose email subject is on an account the user can't read" do
      readable_account = create(:email_account, workspace: workspace)
      create(:email_account_user, :viewer, user: user, email_account: readable_account)
      hidden_account = create(:email_account, workspace: workspace)

      readable_email = create(:email_message, email_account: readable_account)
      hidden_email = create(:email_message, email_account: hidden_account)

      visible = create(:event, workspace: workspace, name: "email.received", subject: readable_email)
      hidden = create(:event, workspace: workspace, name: "email.received", subject: hidden_email)
      non_email = create(:event, workspace: workspace, name: "document.approved", subject: create(:document, workspace: workspace))

      result = Event.accessible_to(user)
      expect(result).to include(visible, non_email)
      expect(result).not_to include(hidden)
    end
  end
end
