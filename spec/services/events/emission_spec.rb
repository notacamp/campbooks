require "rails_helper"

# Representative coverage that the domain chokepoints actually publish events.
# The full matrix lives in Events::Registry; these pin the wiring at the model
# layer (the rest is covered by their own service/request specs).
RSpec.describe "Domain event emission", type: :model do
  describe "Contact sender-list actions" do
    let(:contact) { create(:contact) }

    it "emits contact.starred / contact.unstarred" do
      expect { contact.star! }.to change { Event.named("contact.starred").count }.by(1)
      expect { contact.unstar! }.to change { Event.named("contact.unstarred").count }.by(1)

      event = Event.named("contact.starred").last
      expect(event.subject).to eq(contact)
      expect(event.workspace).to eq(contact.workspace)
      expect(event.payload).to include("email" => contact.email)
    end

    it "emits contact.blocked / contact.unblocked" do
      expect { contact.block! }.to change { Event.named("contact.blocked").count }.by(1)
      expect { contact.unblock! }.to change { Event.named("contact.unblocked").count }.by(1)
    end
  end

  describe "Document review actions" do
    let(:user) { create(:user) }
    let(:document) { create(:document, workspace: user.workspace) }

    it "emits document.approved with the reviewer as actor" do
      expect { document.approve!(by: user) }.to change { Event.named("document.approved").count }.by(1)
      event = Event.named("document.approved").last
      expect(event.subject).to eq(document)
      expect(event.actor).to eq(user)
    end

    it "emits document.rejected and document.restored" do
      document.approve!(by: user)
      expect { document.reject! }.to change { Event.named("document.rejected").count }.by(1)
      expect { document.restore! }.to change { Event.named("document.restored").count }.by(1)
    end
  end
end
