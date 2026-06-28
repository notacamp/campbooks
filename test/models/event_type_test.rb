require "test_helper"

class EventTypeTest < ActiveSupport::TestCase
  setup do
    @ws = Workspace.create!(name: "EventType Test WS")
  end

  test "valid with a name and color" do
    assert @ws.event_types.new(name: "Meeting", color: "#5484ed").valid?
  end

  test "requires a name" do
    type = @ws.event_types.new(color: "#5484ed")
    assert_not type.valid?
    assert type.errors[:name].any?
  end

  test "requires a color" do
    type = @ws.event_types.new(name: "Meeting")
    assert_not type.valid?
    assert type.errors[:color].any?
  end

  test "name is unique within a workspace" do
    @ws.event_types.create!(name: "Meeting", color: "#5484ed")
    dup = @ws.event_types.new(name: "Meeting", color: "#dc2127")
    assert_not dup.valid?
    assert dup.errors[:name].any?
  end

  test "the same name is allowed in a different workspace" do
    @ws.event_types.create!(name: "Meeting", color: "#5484ed")
    other = Workspace.create!(name: "Other EventType WS")
    assert other.event_types.new(name: "Meeting", color: "#dc2127").valid?
  end

  test "prompt stores and reads back as plain text" do
    type = @ws.event_types.create!(name: "Meeting", color: "#5484ed", prompt: "Calls and syncs")
    assert_equal "Calls and syncs", type.reload.prompt
  end

  test "starter types are all valid and use the Google event palette" do
    palette = Calendars::EventColors.palette.map { |c| c[:hex] }
    EventType::STARTERS.each do |attrs|
      type = @ws.event_types.new(attrs)
      assert type.valid?, "#{attrs[:name]}: #{type.errors.full_messages.to_sentence}"
      assert_includes palette, attrs[:color], "#{attrs[:name]} color is outside the palette"
    end
  end
end
