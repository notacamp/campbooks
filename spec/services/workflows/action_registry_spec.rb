require "rails_helper"

RSpec.describe Workflows::ActionRegistry do
  it "registers the known action types" do
    expect(described_class.keys).to contain_exactly(
      "send_email", "http_request", "slack_message", "discord_message", "custom_action", "email_action", "emit_event",
      "google_drive_create_folder", "google_drive_upload", "notion_create_page", "notion_create_database_item"
    )
  end

  it "flags HTTP-backed actions and not in-app ones" do
    expect(described_class.http_keys).to contain_exactly(
      "http_request", "slack_message", "discord_message", "custom_action"
    )
    expect(described_class.definition("send_email").http?).to be(false)
    expect(described_class.definition("slack_message").http?).to be(true)
  end

  it "exposes a unified label for every action" do
    expect(described_class.labels).to eq(
      "send_email" => "Send Email",
      "http_request" => "HTTP Request",
      "slack_message" => "Slack Message",
      "discord_message" => "Discord Message",
      "custom_action" => "Custom Action",
      "email_action" => "Email Action",
      "emit_event" => "Emit Event",
      "google_drive_create_folder" => "Create Drive Folder",
      "google_drive_upload" => "Upload to Drive",
      "notion_create_page" => "Create Notion Page",
      "notion_create_database_item" => "Create Notion Item"
    )
  end

  it "unions every action's config keys (the action half of strong params)" do
    expect(described_class.config_keys).to include(
      "email_account_id", "to_template", "http_method", "url", "headers",
      "content_type", "body", "webhook_url", "text", "content", "username",
      "connection_id", "path"
    )
  end

  it "derives picker cards and select options from the same definitions" do
    expect(described_class.picker_cards.map { |c| c[:action_type] }).to eq(described_class.keys)
    expect(described_class.select_options).to eq(described_class.all.map { |d| [ d.key, d.label ] })
  end

  it "looks up a definition by string key" do
    expect(described_class.definition("http_request").build).to eq(:build_generic_request)
    expect(described_class.definition("nope")).to be_nil
  end

  # The registry <-> executor contract: every build/run symbol must resolve to a
  # real Executor method, or dispatch would silently no-op.
  it "points every action at an executor method that exists" do
    described_class.all.each do |defn|
      method_name = defn.build || defn.run
      defined = Workflows::Executor.private_method_defined?(method_name) ||
                Workflows::Executor.method_defined?(method_name)
      expect(defined).to be(true),
        "expected Workflows::Executor to define ##{method_name} for action '#{defn.key}'"
    end
  end
end
