# frozen_string_literal: true

require "rails_helper"

# The account menu: identity + workspace, Settings (opens the overlay), keyboard
# shortcuts, the Appearance control, Admin (cloud operators) and Sign out. Behaviour
# is driven by the dropdown / theme / settings-overlay / email-shortcuts controllers;
# here we pin the markup hooks they depend on.
RSpec.describe Campbooks::UserMenu, type: :component do
  let(:workspace) { Workspace.create!(name: "Demo Workspace", slug: "demo-#{SecureRandom.hex(4)}") }
  let(:user) do
    workspace.users.create!(name: "Ana Demo", email_address: "ana-#{SecureRandom.hex(4)}@example.com", password: "password123")
  end

  before do
    Current.session = Session.create!(user: user)
    Current.workspace = workspace
  end
  after { Current.reset }

  around do |example|
    previous = Rails.application.config.self_hosted
    example.run
    Rails.application.config.self_hosted = previous
  end

  def render_menu(**opts)
    ApplicationController.render(described_class.new(**{ open: true }.merge(opts)), layout: false)
  end

  it "shows who you are and which workspace you are in" do
    html = render_menu
    expect(html).to include("Ana Demo")
    expect(html).to include(user.email_address)
    expect(html).to include("Demo Workspace")
  end

  it "opens the settings overlay from the Settings row while keeping a real href" do
    html = render_menu
    expect(html).to include('href="/settings/account"')
    expect(html).to include("settings-overlay#open")
    expect(html).to include('data-settings-overlay-url-param="/settings/account"')
  end

  it "offers the three appearance modes through the theme controller" do
    html = render_menu
    %w[light dark system].each { |mode| expect(html).to include(%(data-theme-mode-param="#{mode}")) }
    expect(html).to include("theme#set")
  end

  it "opens the keyboard shortcuts cheat sheet through a Stimulus action, not an inline handler" do
    html = render_menu
    expect(html).to include("email-shortcuts#showHelp")
    expect(html).not_to include("onclick")
  end

  it "signs out through a DELETE form with the icon rendered as markup" do
    html = render_menu
    expect(html).to include('action="/session"')
    expect(html).to include('value="delete"')
    expect(html).to include(I18n.t("shared.user_menu.sign_out"))
    expect(html).not_to include("&lt;svg")
  end

  it "shows the running version to self-hosters and the beta chip on the cloud" do
    Rails.application.config.self_hosted = true
    expect(render_menu).to include("v#{Campbooks::VERSION}")

    Rails.application.config.self_hosted = false
    html = render_menu
    expect(html).to include(I18n.t("components.user_menu.beta"))
    expect(html).not_to include("v#{Campbooks::VERSION}")
  end

  it "shows Admin only to cloud app admins" do
    Rails.application.config.self_hosted = false
    expect(render_menu).not_to include('href="/admin"')

    user.update!(app_admin: true)
    expect(render_menu).to include('href="/admin"')
  end

  it "renders the popover beside the rail and the sheet from the bottom with a scrim" do
    expect(render_menu(variant: :popover)).to include("left-full")

    sheet = render_menu(variant: :sheet)
    expect(sheet).to include('data-dropdown-target="scrim"')
    expect(sheet).to include("inset-x-0 bottom-0")
  end

  it "keeps the panel hidden until the dropdown opens it" do
    html = render_menu(open: false)
    panel = html[/<div[^>]*data-dropdown-target="panel"[^>]*>/]
    expect(panel).to include("hidden")
    expect(render_menu(open: true)[/<div[^>]*data-dropdown-target="panel"[^>]*>/]).not_to include("hidden")
  end
end
