require "rails_helper"

RSpec.describe Events::Registry do
  it "indexes definitions by key with no duplicates" do
    keys = described_class.keys
    expect(keys).to eq(keys.uniq)
    expect(described_class.definition("document.approved").label).to eq("Document approved")
  end

  it "exposes select options and grouped options for the picker" do
    expect(described_class.select_options).to include([ "contact.starred", "Contact starred" ])
    expect(described_class.grouped_select_options[:email]).to be_present
  end

  it "returns payload keys for a known event and an empty list otherwise" do
    expect(described_class.payload_keys_for("email.received")).to include("subject")
    expect(described_class.payload_keys_for("nope.unknown")).to eq([])
  end

  it "gives every definition a label, group, icon and description" do
    described_class.all.each do |definition|
      expect(definition.label).to be_present, "#{definition.key} missing label"
      expect(definition.group).to be_present, "#{definition.key} missing group"
      expect(definition.icon).to be_present, "#{definition.key} missing icon"
      expect(definition.description).to be_present, "#{definition.key} missing description"
    end
  end

  it "builds an i18n-safe key fragment" do
    expect(described_class.definition("email.received").i18n_key).to eq("email_received")
  end
end
