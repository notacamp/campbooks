# frozen_string_literal: true

module Campbooks
  # The settings overlay's content frame: <turbo-frame id="settings_panel"> carrying
  # a settings page (group crumb + inline flash + the page itself) or, when nothing
  # is loaded yet, a skeleton. Rendered by Campbooks::SettingsOverlay on full page
  # renders and by layouts/settings_frame for Turbo-Frame requests, so a page looks
  # the same however it arrived in the pane.
  #
  # @param current_section [String, nil] the settings section key ("account",
  #   "inbox_rules"…) used to find the group for the crumb
  # @param content [String, nil] the captured settings page HTML (html_safe)
  # @param current_url [String, nil] the page URL, stamped on the frame so the
  #   settings-overlay controller knows the frame already holds this page
  # @param context [Settings::Catalog::Context, nil] visibility context for the crumb
  class SettingsPanel < Campbooks::Base
    register_element :turbo_frame, tag: "turbo-frame"

    INNER_CLASSES = "settings-overlay-inner mx-auto w-full max-w-[720px] px-5 py-6 lg:px-10 lg:py-9"

    def initialize(current_section: nil, content: nil, current_url: nil, context: nil)
      @current_section = current_section
      @content = content
      @current_url = current_url
      @context = context
    end

    def view_template
      data = { turbo_action: "advance", settings_overlay_target: "frame" }
      data[:current_url] = @current_url if loaded? && @current_url

      turbo_frame(id: "settings_panel", class: "block min-h-0 flex-1 overflow-y-auto", data: data) do
        loaded? ? page : skeleton
      end
    end

    private

    def loaded?
      @content.present?
    end

    def page
      div(class: INNER_CLASSES) do
        crumb
        raw safe(helpers.render("shared/flash_inline").to_s)
        raw safe(@content.to_s)
      end
    end

    # The group the current page belongs to, as an eyebrow above the page's own title.
    def crumb
      return if @current_section.blank?

      context = @context || Settings::Catalog::Context.new(user: nil, native: false)
      item = Settings::Catalog.item_for_section(@current_section, context)
      return unless item

      group = Settings::Catalog.groups(context).find { |g| g.items.include?(item) }
      return unless group

      p(class: "mb-1.5 text-[11px] font-semibold uppercase tracking-[0.06em] text-muted-foreground") do
        plain I18n.t("settings.catalog.groups.#{group.key}")
      end
    end

    def skeleton
      div(class: INNER_CLASSES, aria: { busy: "true" }) do
        div(class: "mb-6 h-6 w-40 animate-pulse rounded bg-muted")
        div(class: "mb-3 h-3 w-full animate-pulse rounded bg-muted")
        div(class: "mb-3 h-3 w-4/5 animate-pulse rounded bg-muted")
        div(class: "h-3 w-2/3 animate-pulse rounded bg-muted")
      end
    end
  end
end
