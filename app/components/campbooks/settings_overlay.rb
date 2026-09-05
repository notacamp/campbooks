# frozen_string_literal: true

module Campbooks
  # Global settings overlay. Renders as a <dialog> over the page.
  #
  # Mounted once in each layout (application + email) for authenticated users,
  # next to the other global dialogs (after ScoutOverlay). The settings-overlay
  # Stimulus controller (on <body>) drives open/close/navigation.
  #
  # @param current_section [String, nil] the current settings section key (e.g.
  #   "account", "inbox_rules"). Used to highlight the active nav item.
  # @param content [String, nil] the captured settings page HTML (nil when the
  #   overlay is rendered on a non-settings page — shows a skeleton placeholder).
  # @param context [Settings::Catalog::Context, nil] visibility context
  class SettingsOverlay < Campbooks::Base
    def initialize(current_section: nil, content: nil, context: nil)
      @current_section = current_section
      @content         = content
      @context         = context || default_context
    end

    def view_template
      dialog(
        id: "settings-overlay",
        data: {
          settings_overlay_target: "dialog",
          action: "close->settings-overlay#onClose click->settings-overlay#backdropClose"
        },
        aria: { label: t(".title") },
        class: [
          "settings-overlay m-auto h-full w-full max-h-none max-w-none bg-transparent p-0",
          "backdrop:bg-black/40 dark:backdrop:bg-black/60",
          "lg:h-[min(660px,calc(100vh-56px))] lg:w-[min(1000px,calc(100vw-56px))]"
        ].join(" ")
      ) do
        div(class: "flex h-full w-full flex-col overflow-hidden bg-background text-foreground lg:flex-row lg:rounded-[20px] lg:border lg:border-border lg:shadow-2xl") do
          nav_aside
          content_section
        end
      end
    end

    private

    def nav_aside
      aside(
        data: { settings_overlay_target: "nav" },
        class: "flex min-h-0 w-full flex-col bg-sidebar lg:w-[248px] lg:flex-none lg:border-r lg:border-border"
      ) do
        nav_head
        find_box
        groups_nav
      end
    end

    def nav_head
      div(class: "flex items-center gap-2 px-4 pt-4 pb-2.5 lg:pt-[18px]") do
        h2(class: "flex-1 text-[15px] font-semibold tracking-[-0.01em] max-lg:text-center max-lg:text-xl") { t(".title") }
        button(
          type: "button",
          aria: { label: t(".close") },
          data: { action: "click->settings-overlay#close" },
          class: "lg:hidden inline-flex size-8 items-center justify-center rounded-[9px] text-muted-foreground hover:bg-muted hover:text-foreground"
        ) { raw safe(icon_svg(:x, "w-5 h-5")) }
      end
    end

    def find_box
      div(class: "mx-3 mb-2.5 flex h-[34px] items-center gap-2 rounded-[10px] border border-border bg-card px-2.5") do
        raw safe(icon_svg(:search, "w-[14px] h-[14px] text-muted-foreground flex-none"))
        input(
          type: "search",
          placeholder: t(".find"),
          aria: { label: t(".find") },
          data: { settings_overlay_target: "find", action: "input->settings-overlay#filter" },
          class: "min-w-0 flex-1 border-0 bg-transparent p-0 text-[13px] focus:outline-none focus:ring-0"
        )
        span(class: "hidden lg:inline-flex font-mono text-[11px] leading-none text-muted-foreground bg-muted border border-border rounded-[5px] px-1.5 py-[3px]") { plain "/" }
      end
    end

    def groups_nav
      nav(
        aria: { label: t(".sections") },
        class: "flex min-h-0 flex-col gap-3 overflow-y-auto px-2 pb-4 max-lg:pb-24"
      ) do
        Settings::Catalog.groups(@context).each { |group| render_group(group) }
      end
    end

    def render_group(group)
      div(data: { settings_overlay_target: "group" }) do
        h3(class: "mx-2 mb-0.5 mt-1.5 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.06em] text-muted-foreground") do
          span(class: "size-1.5 rounded-full bg-ember-gradient") if group.ember
          plain I18n.t("settings.catalog.groups.#{group.key}")
        end
        group.items.each { |item| render_nav_item(item, group) }
      end
    end

    def render_nav_item(item, group)
      active = active_item?(item)
      a(
        href: item[:path],
        data: {
          turbo_frame: "settings_panel",
          settings_overlay_target: "navItem",
          action: "click->settings-overlay#navigate",
          active_keys: item[:active_keys].join(" ")
        },
        aria: { current: active ? "page" : nil },
        class: "flex h-8 w-full items-center gap-2.5 rounded-lg px-2 text-[13.5px] font-medium text-muted-foreground hover:bg-muted hover:text-foreground aria-[current=page]:bg-secondary aria-[current=page]:text-foreground max-lg:h-11 max-lg:text-[15px] max-lg:font-normal max-lg:text-foreground"
      ) do
        raw safe(Settings::Catalog.icon_svg(item[:icon], css: "w-4 h-4 flex-none"))
        span(class: "flex-1 truncate") { plain item[:label] }
        raw safe(icon_svg(:"chevron-right", "w-[14px] h-[14px] text-muted-foreground lg:hidden"))
      end
    end

    def content_section
      section(
        data: { settings_overlay_target: "pane" },
        class: "relative flex min-h-0 min-w-0 flex-1 flex-col"
      ) do
        phone_back_button
        close_button
        content_frame
      end
    end

    def phone_back_button
      div(class: "lg:hidden flex h-12 items-center gap-1 border-b border-border px-2") do
        button(
          type: "button",
          aria: { label: t(".back") },
          data: { action: "click->settings-overlay#back" },
          class: "inline-flex items-center justify-center rounded-lg p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
        ) { raw safe(icon_svg(:"chevron-left", "w-5 h-5")) }
        span(class: "text-sm font-medium text-foreground") { t(".title") }
      end
    end

    def close_button
      button(
        type: "button",
        aria: { label: t(".close") },
        data: { action: "click->settings-overlay#close" },
        class: "hidden lg:grid absolute right-3.5 top-3.5 z-10 size-8 place-items-center rounded-[9px] text-muted-foreground hover:bg-muted hover:text-foreground"
      ) { raw safe(icon_svg(:x, "w-4 h-4")) }
    end

    def content_frame
      current_url_attr = @content.present? ? { current_url: helpers.request.original_url } : {}
      raw safe(
        "<turbo-frame id=\"settings_panel\" " \
        "data-turbo-action=\"advance\" " \
        "data-settings-overlay-target=\"frame\" " \
        "#{current_url_attr.map { |k, v| "data-#{k.to_s.dasherize}=\"#{v}\"" }.join(' ')} " \
        "class=\"block min-h-0 flex-1 overflow-y-auto\">" \
        "#{frame_content}" \
        "</turbo-frame>"
      )
    end

    def frame_content
      if @content.present?
        crumb_html = crumb_markup
        "<div class=\"mx-auto w-full max-w-[720px] px-5 py-6 lg:px-10 lg:py-9\">#{crumb_html}#{flash_inline_html}#{@content}</div>"
      else
        skeleton_html
      end
    end

    def crumb_markup
      return "" if @current_section.blank?

      item = Settings::Catalog.item_for_section(@current_section, @context)
      return "" unless item

      group_key = Settings::Catalog.groups(@context).find { |g| g.items.include?(item) }&.key
      return "" unless group_key

      label = I18n.t("settings.catalog.groups.#{group_key}")
      "<p class=\"mb-1.5 text-[11px] font-semibold uppercase tracking-[0.06em] text-muted-foreground\">#{CGI.escapeHTML(label)}</p>"
    end

    def flash_inline_html
      # Render the flash partial for inline display inside the frame
      begin
        helpers.render("shared/flash_inline")
      rescue
        ""
      end
    end

    def skeleton_html
      <<~HTML
        <div class="mx-auto w-full max-w-[720px] px-5 py-6 lg:px-10 lg:py-9">
          <div class="h-6 w-40 animate-pulse rounded bg-muted mb-6"></div>
          <div class="h-3 w-full animate-pulse rounded bg-muted mb-3"></div>
          <div class="h-3 w-4/5 animate-pulse rounded bg-muted mb-3"></div>
          <div class="h-3 w-2/3 animate-pulse rounded bg-muted"></div>
        </div>
      HTML
    end

    def active_item?(item)
      return false if @current_section.blank?
      item[:active_keys].include?(@current_section.to_s)
    end

    def default_context
      Settings::Catalog::Context.new(
        user:   begin
                  helpers.current_user
                rescue
                  nil
                end,
        native: begin
                  helpers.hotwire_native_app?
                rescue
                  false
                end
      )
    end

    def icon_svg(name, css_class)
      path = Settings::Catalog::ICON_PATHS[name.to_sym] || ""
      fill_attrs = 'fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"'
      %(<svg class="#{css_class}" viewBox="0 0 24 24" #{fill_attrs} aria-hidden="true">#{path}</svg>)
    end
  end
end
