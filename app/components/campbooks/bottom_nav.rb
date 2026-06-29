# frozen_string_literal: true

module Campbooks
  # Mobile bottom tab bar (Instagram-shaped primary navigation). Shown below the
  # `lg` breakpoint only; the desktop counterpart is Campbooks::NavRail. Both
  # read the same source — NavigationHelper#primary_nav_items — so they never
  # drift on items, order, or active state.
  #
  # The active section reads in near-black ink; Scout is the one Ember element —
  # an inline tab like the others, set apart only by its icon on an Ember-gradient
  # chip. Tapping Scout opens the full Scout surface.
  #
  #   render(Campbooks::BottomNav.new)
  #
  # @param items [Array<Hash>, nil] override the nav items (defaults to
  #   helpers.primary_nav_items); each { key:, label:, path:, ember:, active: }
  class BottomNav < Campbooks::Base
    # On the narrow mobile dock these secondary destinations don't earn a
    # permanent tab — they collapse into a single "More" burger that opens a
    # popover above the bar (the desktop NavRail still shows all of them, since
    # the vertical rail has room). Order here is the order in the menu.
    OVERFLOW_KEYS = %i[tasks workflows contacts activity].freeze

    # Hamburger glyph for the "More" tab, matching the stroked weight of the
    # other nav icons (NavigationHelper#nav_icon_svg).
    MORE_SVG = '<svg class="w-[23px] h-[23px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16"/></svg>'

    def initialize(items: nil, **attrs)
      @items = items
      @attrs = attrs
    end

    def view_template
      nav(
        class: class_names(
          "lg:hidden fixed inset-x-0 bottom-0 z-40 flex items-stretch justify-around gap-1 px-1.5",
          "h-16 pb-[env(safe-area-inset-bottom)]",
          "bg-sidebar/90 backdrop-blur-lg border-t border-border",
          @attrs.delete(:class)
        ),
        aria_label: helpers.t("shared.topbar.main_navigation"),
        **@attrs
      ) do
        dock_items.each { |item| item[:ember] ? scout_tab(item) : tab(item) }
        more_tab if overflow_items.any?
      end
    end

    private

    def items
      @items ||= helpers.primary_nav_items
    end

    # Tabs that keep a permanent slot in the dock (everything that isn't folded
    # into the "More" menu), in their original order.
    def dock_items
      items.reject { |item| OVERFLOW_KEYS.include?(item[:key]) }
    end

    # Items folded into the "More" menu, ordered by OVERFLOW_KEYS.
    def overflow_items
      OVERFLOW_KEYS.filter_map { |key| items.find { |item| item[:key] == key } }
    end

    def tab(item)
      a(
        href: item[:path],
        # In the native app, switch tabs (replace) instead of stacking screens,
        # so each tab is a root with no spurious native back button.
        data: { turbo_action: helpers.hotwire_native_app? ? "replace" : nil },
        aria_current: item[:active] ? "page" : nil,
        class: class_names(
          "relative flex flex-1 flex-col items-center justify-center gap-1 rounded-xl",
          "text-[10px] font-semibold tracking-tight transition-colors",
          item[:active] ? "text-foreground" : "text-muted-foreground hover:text-foreground"
        )
      ) do
        span(class: "relative inline-flex") do
          raw(safe(helpers.nav_icon_svg(item[:key], css_class: "w-[23px] h-[23px]")))
          badge_dot if item[:badge]
        end
        span { item[:label] }
        span(class: "sr-only") { helpers.t("shared.nav.new_items") } if item[:badge]
      end
    end

    # Scout: same shape and size as a regular tab, set apart only by its icon
    # sitting on an Ember-gradient chip — present, but no longer shouting.
    def scout_tab(item)
      a(
        href: item[:path],
        # Anchor for the Scout coachmark (NavigationHelper marks Scout :ember); the
        # home composer pill carries the same marker for the desktop layout.
        data: { turbo_action: helpers.hotwire_native_app? ? "replace" : nil, scout_coach_anchor: "" },
        aria_label: scout_aria_label(item),
        aria_current: item[:active] ? "page" : nil,
        class: class_names(
          "relative flex flex-1 flex-col items-center justify-center gap-1 rounded-xl",
          "text-[10px] font-semibold tracking-tight text-ember transition-colors"
        )
      ) do
        span(class: "relative flex size-[23px] items-center justify-center rounded-lg bg-ember-gradient text-white") do
          raw(safe(helpers.nav_icon_svg(:scout, css_class: "w-4 h-4")))
          badge_dot(scout: true) if item[:badge]
        end
        span { item[:label] }
      end
    end

    # The "More" overflow tab: a burger that opens a small popover above the bar
    # (the `dropdown` controller — same one the avatar menu uses) listing the
    # destinations that don't get a permanent dock slot. Lit near-black when the
    # current page is one of them, so the dock still signals where you are.
    def more_tab
      active = overflow_items.any? { |item| item[:active] }
      badge  = overflow_items.any? { |item| item[:badge] }

      div(class: "relative flex flex-1", data: { controller: "dropdown" }) do
        button(
          type: "button",
          data: { action: "click->dropdown#toggle" },
          aria_haspopup: "menu",
          class: class_names(
            "relative flex w-full flex-col items-center justify-center gap-1 rounded-xl cursor-pointer",
            "text-[10px] font-semibold tracking-tight transition-colors",
            active ? "text-foreground" : "text-muted-foreground hover:text-foreground"
          )
        ) do
          span(class: "relative inline-flex") do
            raw(safe(MORE_SVG))
            badge_dot if badge
          end
          span { helpers.t("shared.nav.more") }
        end

        # Popover anchored above the tab, aligned to the right edge so it never
        # clips off-screen. Hidden until the dropdown controller opens it; an
        # outside tap (or Escape) closes it, and tapping a row lets Turbo
        # navigate, which re-renders the dock closed.
        div(
          class: class_names(
            "absolute bottom-full right-0 mb-2 z-50 hidden min-w-[12rem]",
            "rounded-2xl border border-border bg-popover text-popover-foreground p-1.5 shadow-lg"
          ),
          role: "menu",
          aria_label: helpers.t("shared.nav.more"),
          data: { dropdown_target: "panel" }
        ) do
          overflow_items.each { |item| more_menu_link(item) }
        end
      end
    end

    # One row in the "More" popover: nav icon + label, lit when it's the current
    # page (mirrors the dock/rail active treatment).
    def more_menu_link(item)
      a(
        href: item[:path],
        role: "menuitem",
        aria_current: item[:active] ? "page" : nil,
        class: class_names(
          "flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm transition-colors",
          item[:active] ? "bg-muted font-semibold text-foreground" : "text-foreground hover:bg-muted"
        )
      ) do
        span(class: item[:active] ? "text-foreground" : "text-muted-foreground") do
          raw(safe(helpers.nav_icon_svg(item[:key], css_class: "w-5 h-5")))
        end
        span { item[:label] }
      end
    end

    # Small attention dot at the top-right of a tab icon (Navigation::Attention).
    # Ember by default; near-black on Scout, whose icon sits on an Ember chip. The
    # ring cuts it cleanly out of the bar background.
    def badge_dot(scout: false)
      span(
        class: class_names(
          "absolute -right-1 -top-1 size-2 rounded-full ring-2 ring-sidebar",
          scout ? "bg-foreground" : "bg-ember"
        ),
        aria_hidden: "true"
      )
    end

    # Scout's icon carries no visible label text (the tab is aria-labelled), so
    # the "new" hint rides on the aria-label instead of an sr-only span.
    def scout_aria_label(item)
      item[:badge] ? "#{item[:label]}, #{helpers.t('shared.nav.new_items')}" : item[:label]
    end
  end
end
