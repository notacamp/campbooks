# frozen_string_literal: true

require "rails_helper"

RSpec.describe CommandPaletteCatalog do
  let(:ws) { Workspace.create!(name: "Palette WS", slug: "palette-#{SecureRandom.hex(4)}") }
  let(:user) do
    ws.users.create!(
      name: "Palette Tester",
      email_address: "palette-#{SecureRandom.hex(4)}@example.com",
      password: "password123"
    )
  end

  subject(:commands) { described_class.for(user) }

  it "returns an array of command hashes" do
    expect(commands).to be_an(Array)
    expect(commands).to all(include(:id, :name, :category, :icon, :url))
  end

  describe "settings commands" do
    let(:settings_commands) do
      cat = I18n.t("command_palette.categories.settings")
      commands.select { |c| c[:category] == cat }
    end

    it "includes at least one settings command" do
      expect(settings_commands).not_to be_empty
    end

    it "includes the account settings command" do
      expect(settings_commands.map { |c| c[:id] }).to include("settings-account")
    end

    it "does not include inbox_settings= query param in any URL" do
      settings_commands.each do |cmd|
        expect(cmd[:url]).not_to include("inbox_settings="),
          "#{cmd[:id]} URL #{cmd[:url]} should not include inbox_settings="
      end
    end

    it "derives settings commands from Settings::Catalog" do
      context = Settings::Catalog::Context.new(user: user, native: false)
      catalog_ids = Settings::Catalog.groups(context).flat_map do |group|
        group.items.map { |item| "settings-#{item[:key]}" }
      end
      # Every catalog item should appear in the palette
      catalog_ids.each do |id|
        expect(settings_commands.map { |c| c[:id] }).to include(id),
          "Expected settings command #{id} to be present"
      end
    end
  end

  describe "navigate commands" do
    let(:navigate_commands) do
      cat = I18n.t("command_palette.categories.navigate")
      commands.select { |c| c[:category] == cat }
    end

    it "includes the email-scans command pointing to settings_inbox_section" do
      scan_cmd = navigate_commands.find { |c| c[:id] == "email-scans" }
      expect(scan_cmd).to be_present
      # Points to the inbox section now, not the old inbox_settings= URL
      expect(scan_cmd[:url]).to include("/settings/inbox")
    end
  end
end
