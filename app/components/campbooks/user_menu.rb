# frozen_string_literal: true

module Campbooks
  # First-class identity panel / account menu.
  # :popover — used by NavRail (desktop): a fixed panel beside the 80px rail,
  #             bottom-aligned with the avatar that opened it
  # :sheet    — used by the mobile topbar (bottom sheet)
  #
  # The panel is hidden by default (driven by the `dropdown` Stimulus
  # controller). Pass `open: true` in previews to make it visible.
  class UserMenu < Campbooks::Base
    ROW_CLASSES_POPOVER = "flex w-full items-center gap-3 rounded-[10px] px-2.5 text-left text-sm text-foreground hover:bg-muted cursor-pointer h-10"
    ROW_CLASSES_SHEET   = "flex w-full items-center gap-3 rounded-[10px] px-2.5 text-left text-[15px] text-foreground hover:bg-muted cursor-pointer h-11"
    KEYCAP_CLASSES      = "ml-auto font-mono text-[11px] leading-none text-muted-foreground bg-muted border border-border rounded-[5px] px-1.5 py-[3px]"
    ICON_CLASSES        = "w-[18px] h-[18px] text-muted-foreground flex-none"

    def initialize(variant: :popover, open: false)
      @variant = variant
      @open = open
    end

    def view_template
      div(class: "relative inline-flex", data: { controller: "dropdown" }) do
        avatar_trigger
        scrim if sheet?
        panel
      end
    end

    private

    def sheet? = @variant == :sheet
    def popover? = @variant == :popover

    def row_classes = sheet? ? ROW_CLASSES_SHEET : ROW_CLASSES_POPOVER

    def avatar_trigger
      button(
        type: "button",
        aria: { haspopup: "menu", expanded: "false", label: t(".open") },
        data: { dropdown_target: "trigger", action: "click->dropdown#toggle" },
        class: "inline-flex focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-full"
      ) do
        render Campbooks::Avatar.new(name: user_name, size: :md)
      end
    end

    def scrim
      div(
        class: "fixed inset-0 z-40 bg-black/40 hidden",
        data: { dropdown_target: "scrim", action: "click->dropdown#close" }
      )
    end

    def panel
      panel_classes = if sheet?
        "fixed z-50 inset-x-0 bottom-0 rounded-t-[22px] border-t border-border bg-popover text-popover-foreground px-2.5 pt-2 pb-[calc(env(safe-area-inset-bottom)+14px)] shadow-lg"
      else
        "fixed z-50 bottom-3 left-[88px] w-[296px] rounded-2xl border border-border bg-popover text-popover-foreground p-1.5 shadow-lg"
      end

      div(
        class: class_names(panel_classes, @open ? nil : "hidden"),
        data: { dropdown_target: "panel" },
        role: "menu",
        aria: { label: t(".label") }
      ) do
        div(class: "mx-auto mb-2 h-1 w-9 rounded-full bg-border") if sheet?
        identity_row
        workspace_row
        separator
        settings_row
        shortcuts_row
        separator
        appearance_row
        separator
        admin_row if show_admin?
        sign_out_row
      end
    end

    def identity_row
      div(class: "flex items-center gap-3 px-2.5 pt-2.5 pb-3") do
        render Campbooks::Avatar.new(name: user_name, size: :lg)
        div do
          b(class: "block text-sm font-semibold leading-tight text-foreground") { plain user_name }
          span(class: "block text-[12.5px] text-muted-foreground") { plain helpers.current_user&.email_address }
        end
      end
    end

    def workspace_row
      ws_name = Current.workspace&.name
      div(class: "mx-1.5 mb-1 flex items-center gap-2 rounded-[10px] bg-secondary px-2.5 py-2 text-[12.5px]") do
        span(class: "inline-flex size-[18px] rounded-[5px] bg-foreground text-background text-[10px] font-semibold items-center justify-center flex-none") do
          plain (ws_name&.first || "W").upcase
        end
        b(class: "flex-1 font-medium truncate text-foreground") { plain ws_name || t(".your_workspace") }
        if helpers.self_hosted?
          a(
            href: "https://github.com/notacamp/campbooks/blob/main/CHANGELOG.md",
            target: "_blank",
            rel: "noopener",
            class: "text-muted-foreground hover:text-foreground text-[12.5px]"
          ) { plain "v#{Campbooks::VERSION}" }
        else
          span(class: "text-muted-foreground") { t(".beta") }
        end
      end
    end

    def separator
      div(class: "my-1.5 h-px bg-border/70 mx-1")
    end

    def settings_row
      a(
        href: helpers.settings_account_path,
        role: "menuitem",
        data: { action: "click->dropdown#close click->settings-overlay#open", "settings-overlay-url-param": helpers.settings_account_path },
        class: row_classes
      ) do
        raw safe(icon_svg(:sliders, ICON_CLASSES))
        span(class: "flex-1") { t(".settings") }
        keycap("⌘ ,", extra_class: "hidden lg:inline-flex")
      end
    end

    def shortcuts_row
      button(
        type: "button",
        role: "menuitem",
        data: { action: "click->dropdown#close click->email-shortcuts#showHelp" },
        class: class_names(row_classes, "hidden lg:flex")
      ) do
        raw safe(icon_svg(:keyboard, ICON_CLASSES))
        span(class: "flex-1") { t("shared.topbar.keyboard_shortcuts") }
        keycap("?")
      end
    end

    def appearance_row
      div(class: "flex items-center gap-3 px-2.5 py-1.5") do
        raw safe(icon_svg(:contrast, ICON_CLASSES))
        span(class: "flex-1 text-sm text-foreground") { t(".appearance") }
        div(
          class: "inline-flex rounded-[9px] bg-secondary p-[2px] gap-[2px]",
          data: { controller: "theme" }
        ) do
          theme_option(:light, :sun, t(".theme_light"))
          theme_option(:dark, :moon, t(".theme_dark"))
          theme_option(:system, :monitor, t(".theme_system"))
        end
      end
    end

    def theme_option(mode, icon_name, label)
      button(
        type: "button",
        aria: { pressed: "false" },
        title: label,
        data: {
          theme_target: "option",
          "theme-mode-param": mode,
          action: "click->theme#set"
        },
        class: "inline-flex items-center gap-1.5 rounded-[7px] px-2 py-1 text-[12px] font-medium text-muted-foreground aria-pressed:bg-card aria-pressed:text-foreground aria-pressed:shadow-sm"
      ) do
        raw safe(mini_icon_svg(icon_name))
        span(class: sheet? ? nil : "sr-only") { plain label }
      end
    end

    def admin_row
      a(
        href: helpers.admin_root_path,
        role: "menuitem",
        class: class_names(row_classes, "text-muted-foreground")
      ) do
        raw safe(icon_svg(:shield, ICON_CLASSES))
        span(class: "flex-1") { t("shared.nav.admin") }
      end
    end

    def sign_out_row
      form(action: helpers.session_path, method: "post", class: "contents") do
        input(type: "hidden", name: "_method", value: "delete", autocomplete: "off")
        if helpers.protect_against_forgery?
          input(type: "hidden", name: helpers.request_forgery_protection_token, value: helpers.form_authenticity_token, autocomplete: "off")
        end
        button(type: "submit", role: "menuitem", class: class_names(row_classes, "text-muted-foreground")) do
          raw safe(icon_svg(:logout, ICON_CLASSES))
          span(class: "flex-1") { t("shared.user_menu.sign_out") }
        end
      end
    end

    def keycap(label, extra_class: nil)
      span(class: class_names(KEYCAP_CLASSES, extra_class)) { plain label }
    end

    # Lookbook renders the menu without a session; the app always has one.
    def user_name
      helpers.current_user&.name || "Preview"
    end

    def show_admin?
      !helpers.self_hosted? && Current.user&.app_admin?
    end

    def icon_svg(name, css_class)
      path = Settings::Catalog::ICON_PATHS[name.to_sym] || ""
      fill_attrs = name.to_sym == :spark ? 'fill="currentColor"' : 'fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"'
      %(<svg class="#{css_class}" viewBox="0 0 24 24" #{fill_attrs} aria-hidden="true">#{path}</svg>)
    end

    def mini_icon_svg(name)
      path = Settings::Catalog::ICON_PATHS[name.to_sym] || ""
      fill_attrs = 'fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"'
      %(<svg class="w-[13px] h-[13px]" viewBox="0 0 24 24" #{fill_attrs} aria-hidden="true">#{path}</svg>)
    end
  end
end
