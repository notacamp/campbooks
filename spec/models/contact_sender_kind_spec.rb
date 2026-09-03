# frozen_string_literal: true

require "rails_helper"

RSpec.describe Contact, type: :model do
  describe "sender_kind" do
    it "defaults to person" do
      expect(create(:contact).sender_kind).to eq("person")
      expect(create(:contact)).to be_kind_person
    end

    it "exposes the kind-prefixed enum and scopes" do
      service = create(:contact, sender_kind: :service)
      expect(service).to be_kind_service
      expect(Contact.kind_service).to include(service)
      expect(Contact.kind_person).not_to include(service)
    end

    it "reports the taught source" do
      expect(create(:contact, sender_kind_source: "taught")).to be_sender_kind_taught
      expect(create(:contact, sender_kind_source: "heuristic")).not_to be_sender_kind_taught
      expect(create(:contact)).not_to be_sender_kind_taught
    end
  end
end
