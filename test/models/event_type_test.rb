require "test_helper"

class EventTypeTest < ActiveSupport::TestCase
  setup do
    @ws = Workspace.create!(name: "EventType Test WS")
  end

  test "valid with just a name (icon is optional)" do
    assert @ws.event_types.new(name: "Meeting").valid?
  end

  test "requires a name" do
    type = @ws.event_types.new
    assert_not type.valid?
    assert type.errors[:name].any?
  end

  test "icon must come from the app-wide icon set" do
    type = @ws.event_types.new(name: "Meeting", icon: "not-a-real-glyph")
    assert_not type.valid?
    assert type.errors[:icon].any?

    assert @ws.event_types.new(name: "Sync", icon: "users").valid?
  end

  test "name is unique within a workspace" do
    @ws.event_types.create!(name: "Meeting")
    dup = @ws.event_types.new(name: "Meeting")
    assert_not dup.valid?
    assert dup.errors[:name].any?
  end

  test "the same name is allowed in a different workspace" do
    @ws.event_types.create!(name: "Meeting")
    other = Workspace.create!(name: "Other EventType WS")
    assert other.event_types.new(name: "Meeting").valid?
  end

  test "prompt stores and reads back as plain text" do
    type = @ws.event_types.create!(name: "Meeting", prompt: "Calls and syncs")
    assert_equal "Calls and syncs", type.reload.prompt
  end

  test "starter types are all valid and use icons from the app-wide set" do
    EventType::STARTERS.each do |attrs|
      type = @ws.event_types.new(attrs)
      assert type.valid?, "#{attrs[:name]}: #{type.errors.full_messages.to_sentence}"
      assert_includes Campbooks::Icon::NAMES, attrs[:icon], "#{attrs[:name]} icon is outside the icon set"
    end
  end
end
