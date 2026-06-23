# frozen_string_literal: true

module Campbooks
  # Desktop left navigation rail (Instagram-web shaped). Shown at `lg` and up
  # only; the mobile counterpart is Campbooks::BottomNav. Both read the same
  # source — NavigationHelper#primary_nav_items — so they never drift.
  #
  # Top to bottom: Ember logo mark → primary destinations (active = near-black
  # ink pill; Scout = the one Ember tile) → spacer → search, notifications, and
  # the avatar menu. It is `position: fixed`, so layouts offset their content
  # with `lg:pl-20` to clear it.
  #
  #   render(Campbooks::NavRail.new)
  #
  # @param items [Array<Hash>, nil] override nav items (defaults to
  #   helpers.primary_nav_items)
  class NavRail < Campbooks::Base
    SEARCH_SVG = '<svg class="w-[21px] h-[21px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>'
    BELL_SVG = '<svg class="w-[21px] h-[21px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M6 8a6 6 0 0 1 12 0c0 7 3 7 3 9H3c0-2 3-2 3-9"/><path d="M10 21a2 2 0 0 0 4 0"/></svg>'

    def initialize(items: nil, **attrs)
      @items = items
      @attrs = attrs
    end

    def view_template
      aside(
        class: class_names(
          "hidden lg:flex fixed inset-y-0 left-0 z-30 w-20 flex-col items-center gap-1.5 py-3",
          "bg-sidebar border-r border-border",
          @attrs.delete(:class)
        ),
        aria_label: helpers.t("shared.topbar.main_navigation"),
        **@attrs
      ) do
        # Brand mark → home
        a(href: helpers.root_path, aria_label: "Campbooks", class: "mb-1.5 inline-flex rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring") do
          render Campbooks::Logo.new(variant: :mark, size: :md, beta: !helpers.self_hosted?)
        end

        items.each { |item| item[:ember] ? scout_item(item) : nav_item(item) }

        div(class: "flex-1")

        # Footer: search · notifications · avatar menu
        button(
          type: "button",
          data: { action: "click->command-palette#open" },
          aria_label: helpers.t("shared.topbar.search_and_commands"),
          class: footer_button_classes
        ) { raw(safe(SEARCH_SVG)) }

        a(href: helpers.notifications_path, aria_label: helpers.t("notifications.bell.heading"), class: class_names(footer_button_classes, "relative")) do
          raw(safe(BELL_SVG))
          if unread_notifications?
            span(class: "absolute right-2.5 top-2 size-2 rounded-full bg-ember", aria_hidden: "true")
          end
        end

        render Campbooks::BugReportButton.new(class: footer_button_classes, icon_class: "w-[21px] h-[21px]")

        avatar
      end
    end

    private

    def items
      @items ||= helpers.primary_nav_items
    end

    def nav_item(item)
      a(
        href: item[:path],
        aria_current: item[:active] ? "page" : nil,
        class: class_names(
          "relative flex w-12 flex-col items-center justify-center gap-1 rounded-xl py-1.5",
          "text-[9px] font-semibold tracking-tight transition-colors",
          item[:active] ? "bg-secondary text-foreground" : "text-muted-foreground hover:bg-muted hover:text-foreground"
        )
      ) do
        span(class: "relative inline-flex") do
          raw(safe(helpers.nav_icon_svg(item[:key], css_class: "w-[22px] h-[22px]")))
          badge_dot if item[:badge]
        end
        span { item[:label] }
        span(class: "sr-only") { helpers.t("shared.nav.new_items") } if item[:badge]
      end
    end

    # Scout: the one Ember element — same w-12 column and formatting as nav_item,
    # set apart only by its icon on an Ember-gradient chip and an Ember-ink label.
    # Mirrors the mobile bottom nav.
    def scout_item(item)
      a(
        href: item[:path],
        aria_label: scout_aria_label(item),
        aria_current: item[:active] ? "page" : nil,
        class: class_names(
          "relative flex w-12 flex-col items-center justify-center gap-1 rounded-xl py-1.5",
          "text-[9px] font-semibold tracking-tight text-ember transition-colors",
          item[:active] ? "bg-secondary" : "hover:bg-muted"
        )
      ) do
        span(class: "relative flex size-[22px] items-center justify-center rounded-lg bg-ember-gradient text-white") do
          raw(safe(helpers.nav_icon_svg(:scout, css_class: "w-4 h-4")))
          badge_dot(scout: true) if item[:badge]
        end
        span { item[:label] }
      end
    end

    # Small attention dot at the top-right of a nav icon (Navigation::Attention).
    # Ember by default; near-black on Scout, whose icon already sits on an Ember
    # chip. The ring cuts it cleanly out of the rail background.
    def badge_dot(scout: false)
      span(
        class: class_names(
          "absolute -right-1 -top-1 size-2 rounded-full ring-2 ring-sidebar",
          scout ? "bg-foreground" : "bg-ember"
        ),
        aria_hidden: "true"
      )
    end

    # Scout's icon carries no visible label text (the link is aria-labelled), so
    # the "new" hint rides on the aria-label instead of an sr-only span.
    def scout_aria_label(item)
      item[:badge] ? "#{item[:label]}, #{helpers.t('shared.nav.new_items')}" : item[:label]
    end

    def avatar
      if helpers.current_user
        raw(safe(helpers.render(partial: "shared/user_menu", locals: { compact: true, placement: :left, drop: :up })))
      else
        # Preview / unauthenticated fallback.
        render Campbooks::Avatar.new(size: :md)
      end
    end

    def footer_button_classes
      "flex size-11 items-center justify-center rounded-xl text-muted-foreground transition-colors hover:bg-muted hover:text-foreground cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    end

    def unread_notifications?
      helpers.current_user&.unread_notifications_count.to_i.positive?
    end
  end
end
