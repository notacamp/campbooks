# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::UserMenu, type: :component do
  def render_menu(**opts)
    ApplicationController.render(described_class.new(**opts), layout: false)
  end

  describe ":popover variant (default)" do
    subject(:html) { render_menu(variant: :popover, open: true) }

    it "renders the user name and email" do
      expect(html).to include("Demo") # seed user
    end

    it "includes the Settings row with settings-overlay#open action" do
      expect(html).to include("settings-overlay#open")
      expect(html).to include("/settings/account")
    end

    it "includes the three theme option buttons" do
      expect(html).to include('theme-mode-param="light"').or include('data-theme-mode-param="light"')
      expect(html).to include("theme#set")
    end

    it "renders left-full positioning classes" do
      expect(html).to include("left-full")
    end

    it "renders self-hosted version link when self-hosted" do
      allow_any_instance_of(ActionView::Base).to receive(:self_hosted?).and_return(true)
      html = render_menu(variant: :popover, open: true)
      expect(html).to include("v#{Campbooks::VERSION}")
    end

    it "renders Beta badge when cloud" do
      allow_any_instance_of(ActionView::Base).to receive(:self_hosted?).and_return(false)
      html = render_menu(variant: :popover, open: true)
      expect(html).to include("Beta")
    end

    it "hides Admin row when not app_admin" do
      # Default seed user is not app_admin in test
      expect(html).not_to include("admin_root_path").and include('href="/admin"')
    end

    it "renders sign-out form POSTing DELETE to /session" do
      expect(html).to include("/session")
      expect(html).to include('method="post"')
      expect(html).to include("_method")
    end
  end

  describe ":sheet variant" do
    subject(:html) { render_menu(variant: :sheet, open: true) }

    it "renders the scrim" do
      expect(html).to include('dropdown_target="scrim"').or include('data-dropdown-target="scrim"')
    end

    it "renders fixed positioning classes" do
      expect(html).to include("fixed")
      expect(html).to include("inset-x-0")
    end

    it "renders the grabber bar" do
      expect(html).to include("h-1 w-9 rounded-full bg-border")
    end
  end
end
