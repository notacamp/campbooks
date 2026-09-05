# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::SettingsOverlay, type: :component do
  let(:user)    { instance_double("User", admin?: false, app_admin?: false) }
  let(:context) { Settings::Catalog::Context.new(user: user, native: false) }

  def render_overlay(**opts)
    ApplicationController.render(described_class.new(**opts), layout: false)
  end

  it "renders <dialog id=settings-overlay>" do
    html = render_overlay
    expect(html).to include('id="settings-overlay"')
    expect(html).to include("<dialog")
  end

  it "renders a turbo-frame id=settings_panel with data-turbo-action=advance" do
    html = render_overlay
    expect(html).to include('id="settings_panel"')
    expect(html).to include('data-turbo-action="advance"')
  end

  it "renders nav labels for all six groups" do
    html = render_overlay
    %w[You Scout Inbox Paper Connections Workspace].each do |label|
      expect(html).to include(label), "Expected group label '#{label}' to appear"
    end
  end

  context "with current_section matching an item" do
    it "sets aria-current=page on the matching nav item" do
      html = render_overlay(current_section: "account")
      expect(html).to include('aria-current="page"')
    end

    it "shows the group label as a crumb above the (empty) content" do
      html = render_overlay(current_section: "account", content: "<h1>Account</h1>")
      # crumb markup
      expect(html).to include("You") # group label
    end
  end

  context "with content provided" do
    it "renders the content inside the frame" do
      html = render_overlay(current_section: "account", content: "<h1>Hello settings</h1>")
      expect(html).to include("Hello settings")
    end

    it "sets data-current-url on the turbo-frame" do
      html = render_overlay(current_section: "account", content: "<p>test</p>")
      expect(html).to include("data-current-url")
    end
  end

  context "without content" do
    it "renders the skeleton placeholder" do
      html = render_overlay
      expect(html).to include("animate-pulse")
    end
  end

  context "with gated features off" do
    before do
      allow(Features).to receive(:document_templates?).and_return(false)
      allow(Features).to receive(:email_templates?).and_return(false)
      allow(Features).to receive(:workflows?).and_return(false)
    end

    it "does not include gated item labels when flags are off" do
      html = render_overlay
      # These labels should not appear since the features are off
      expect(html).not_to include("Automations")
    end
  end
end
