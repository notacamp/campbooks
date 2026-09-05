# frozen_string_literal: true

require "rails_helper"

RSpec.describe Settings::Catalog do
  let(:user)    { instance_double("User", admin?: false, app_admin?: false) }
  let(:context) { Settings::Catalog::Context.new(user: user, native: false) }

  describe ".groups" do
    subject(:groups) { described_class.groups(context) }

    it "returns exactly 6 groups in order" do
      expect(groups.map(&:key)).to eq([:you, :scout, :inbox, :paper, :connections, :workspace])
    end

    it "marks only the scout group as ember" do
      expect(groups.find { |g| g.key == :scout }.ember).to be true
      expect(groups.reject { |g| g.key == :scout }.map(&:ember)).to all(be_falsy)
    end

    it "every visible item has a resolvable path" do
      groups.each do |group|
        group.items.each do |item|
          expect(item[:path]).to be_a(String), "#{item[:key]} has no path"
          expect(item[:path]).to start_with("/"), "#{item[:key]} path '#{item[:path]}' does not start with /"
        end
      end
    end

    context "with gated features off (default)" do
      before do
        allow(Features).to receive(:document_templates?).and_return(false)
        allow(Features).to receive(:email_templates?).and_return(false)
        allow(Features).to receive(:workflows?).and_return(false)
      end

      it "hides document_templates, email_templates, and automations" do
        scout_items = groups.find { |g| g.key == :scout }.items.map { |i| i[:key] }
        expect(scout_items).not_to include(:document_templates, :email_templates, :automations)
      end
    end

    context "with gated features on" do
      before do
        allow(Features).to receive(:document_templates?).and_return(true)
        allow(Features).to receive(:email_templates?).and_return(true)
        allow(Features).to receive(:workflows?).and_return(true)
      end

      it "shows document_templates, email_templates, and automations" do
        scout_items = groups.find { |g| g.key == :scout }.items.map { |i| i[:key] }
        expect(scout_items).to include(:document_templates, :email_templates, :automations)
      end
    end

    context "when native: true" do
      let(:context) { Settings::Catalog::Context.new(user: user, native: true) }

      it "hides api_access" do
        connections_items = groups.find { |g| g.key == :connections }.items.map { |i| i[:key] }
        expect(connections_items).not_to include(:api_access)
      end
    end

    context "when native: false" do
      it "shows api_access" do
        connections_items = groups.find { |g| g.key == :connections }.items.map { |i| i[:key] }
        expect(connections_items).to include(:api_access)
      end
    end

    context "when user is not workspace admin" do
      it "hides system_health" do
        workspace_items = groups.find { |g| g.key == :workspace }.items.map { |i| i[:key] }
        expect(workspace_items).not_to include(:system_health)
      end
    end

    context "when user is workspace admin" do
      let(:user) { instance_double("User", admin?: true, app_admin?: false) }

      it "shows system_health" do
        workspace_items = groups.find { |g| g.key == :workspace }.items.map { |i| i[:key] }
        expect(workspace_items).to include(:system_health)
      end
    end
  end

  describe ".item_for_section" do
    it "returns the rules item for inbox_rules" do
      item = described_class.item_for_section("inbox_rules", context)
      expect(item).not_to be_nil
      expect(item[:key]).to eq(:rules)
    end

    it "returns the security item for totp" do
      item = described_class.item_for_section("totp", context)
      expect(item).not_to be_nil
      expect(item[:key]).to eq(:security)
    end

    it "returns nil for unknown sections" do
      expect(described_class.item_for_section("unknown_section", context)).to be_nil
    end
  end

  describe ".default_path" do
    it "returns the settings account path" do
      expect(described_class.default_path).to eq("/settings/account")
    end
  end

  describe ".icon_svg" do
    it "returns an svg string for a known icon" do
      svg = described_class.icon_svg(:user, css: "w-4 h-4")
      expect(svg).to include("<svg")
      expect(svg).to include("w-4 h-4")
    end
  end
end
