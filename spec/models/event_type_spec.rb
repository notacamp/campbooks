require "rails_helper"

RSpec.describe EventType do
  before do
    @ws = Workspace.create!(name: "EventType Test WS")
  end

  it "valid with just a name (icon is optional)" do
    expect(@ws.event_types.new(name: "Meeting")).to be_valid
  end

  it "requires a name" do
    type = @ws.event_types.new
    expect(type).not_to be_valid
    expect(type.errors[:name]).to be_any
  end

  it "icon must come from the app-wide icon set" do
    type = @ws.event_types.new(name: "Meeting", icon: "not-a-real-glyph")
    expect(type).not_to be_valid
    expect(type.errors[:icon]).to be_any

    expect(@ws.event_types.new(name: "Sync", icon: "users")).to be_valid
  end

  it "name is unique within a workspace" do
    @ws.event_types.create!(name: "Meeting")
    dup = @ws.event_types.new(name: "Meeting")
    expect(dup).not_to be_valid
    expect(dup.errors[:name]).to be_any
  end

  it "the same name is allowed in a different workspace" do
    @ws.event_types.create!(name: "Meeting")
    other = Workspace.create!(name: "Other EventType WS")
    expect(other.event_types.new(name: "Meeting")).to be_valid
  end

  it "prompt stores and reads back as plain text" do
    type = @ws.event_types.create!(name: "Meeting", prompt: "Calls and syncs")
    expect(type.reload.prompt).to eq("Calls and syncs")
  end

  it "starter types are all valid and use icons from the app-wide set" do
    EventType::STARTERS.each do |attrs|
      type = @ws.event_types.new(attrs)
      expect(type).to be_valid, "#{attrs[:name]}: #{type.errors.full_messages.to_sentence}"
      expect(Campbooks::Icon::NAMES).to include(attrs[:icon]), "#{attrs[:name]} icon is outside the icon set"
    end
  end
end
