# frozen_string_literal: true

module Campbooks
  # "Scout can do more": the quiet, collapsible list of remaining setup tasks.
  # It rides along in the feed as one compact card — it never owns the screen,
  # never shows percentages, and finished tasks simply leave the list (when
  # nothing is left the component renders nothing and the hub is gone).
  #
  # At most three tasks show at once, priority first (SetupStatus's order); the
  # current one leads with an ember dot and a primary CTA, the rest stay ghost.
  # Pure/data-driven so it previews without a workspace — the partial extracts
  # the data from SetupStatus (items arrive localized via SetupStatus#all_items).
  #
  # Renders inside the existing #setup_banner wrapper, so the setup modals'
  # completion stream (turbo_stream.replace "setup_banner") keeps refreshing it.
  class SetupHub < Campbooks::Base
    MAX_ROWS = 3

    # @param items [Array<Hash>] all setup tasks (SetupStatus#all_items shape)
    # @param incomplete_keys [Array<Symbol,String>] keys of still-incomplete tasks
    # @param dismissed_keys [Array<String>] keys the user dismissed
    # @param return_to [String] path to return to after a setup modal completes
    # @param collapsed [Boolean] start collapsed (used on dense pages)
    def initialize(items:, incomplete_keys:, dismissed_keys: [], return_to: "/", collapsed: false, **attrs)
      @items = items
      @incomplete_keys = incomplete_keys.map(&:to_sym)
      @dismissed_keys = dismissed_keys.map(&:to_s)
      @return_to = return_to
      @collapsed = collapsed
      @attrs = attrs
    end

    def view_template
      return if open_items.empty?

      custom_class = @attrs.delete(:class)
      details(
        id: "setup_hub", open: !@collapsed,
        class: class_names(
          "group/hub block overflow-hidden rounded-2xl border border-border bg-card text-card-foreground",
          custom_class
        ),
        **@attrs
      ) do
        render_summary
        div(class: "border-t border-border") do
          render_rows
          render_footer
        end
      end
    end

    private

    # Still-to-do tasks the user hasn't dismissed — the hub's whole world.
    # Finished tasks don't linger as strikethrough rows; they just leave.
    def open_items
      @open_items ||= @items.select { |item| incomplete?(item) && !dismissed?(item) }
    end

    def shown_items
      open_items.first(MAX_ROWS)
    end

    def overflow_count
      open_items.size - shown_items.size
    end

    def incomplete?(item)
      @incomplete_keys.include?(item[:key])
    end

    def dismissed?(item)
      @dismissed_keys.include?(item[:key].to_s)
    end

    def current_key
      @current_key ||= shown_items.first&.dig(:key)
    end

    def render_summary
      summary(class: "list-none cursor-pointer select-none px-4 py-3 [&::-webkit-details-marker]:hidden") do
        div(class: "flex items-center gap-3") do
          render Campbooks::ScoutAvatar.new(size: :sm)
          p(class: "min-w-0 flex-1 truncate text-sm text-foreground") do
            span(class: "font-semibold") { t(".heading") }
            span(class: "text-muted-foreground") { " · #{t(".remaining", count: open_items.size)}" }
          end
          span(class: "text-muted-foreground transition-transform duration-200 group-open/hub:rotate-180") do
            raw(safe(chevron_svg))
          end
        end
      end
    end

    def render_rows
      ul(class: "divide-y divide-border") do
        shown_items.each { |item| render_row(item) }
      end
    end

    def render_row(item)
      current = item[:key] == current_key

      li(id: "setup_hub_row_#{item[:key]}", class: "flex items-start gap-3 px-4 py-3.5") do
        span(class: "mt-1 flex h-4 w-4 shrink-0 items-center justify-center") do
          if current
            span(class: "h-2 w-2 rounded-full", style: "background-color: var(--ember-solid)")
          else
            span(class: "h-2 w-2 rounded-full border-[1.5px] border-border")
          end
        end
        div(class: "min-w-0 flex-1") do
          p(class: "text-sm font-semibold text-foreground") { item[:message] }
          p(class: "mt-0.5 text-[13px] leading-snug text-muted-foreground line-clamp-2") { item[:description] }
          div(class: "mt-2.5 flex flex-wrap items-center gap-2") { render_ctas(item, current: current) }
        end
        render_dismiss(item)
      end
    end

    def render_ctas(item, current:)
      case item[:key]
      when :email_account then render_email_ctas
      when :document_types then render_ai_ctas("document_types", item, current: current)
      when :tags then render_ai_ctas("tags", item, current: current)
      else render_modal_cta(item, current: current)
      end
    end

    def render_modal_cta(item, current:, step: item[:key])
      render Campbooks::Button.new(
        variant: current ? :primary : :outline, size: :sm,
        data: { setup_modal_open: helpers.setup_path(step, return_to: @return_to) }
      ) { item[:cta_text] }
    end

    # AI-first for types/tags: lead with Scout drafting them, keep manual entry
    # as the quiet second door.
    def render_ai_ctas(kind, item, current:)
      render Campbooks::Button.new(
        variant: current ? :primary : :outline, size: :sm,
        data: { setup_modal_open: helpers.ai_setup_chat_path(kind: kind) }
      ) { item[:cta_text] }
      render Campbooks::Button.new(
        variant: :ghost, size: :sm,
        data: { setup_modal_open: helpers.setup_path(item[:key], return_to: @return_to) }
      ) { t(".add_manually") }
    end

    # The inbox task connects in place — the same quiet provider cards as the
    # welcome screen, not colored provider buttons.
    def render_email_ctas
      div(class: "grid w-full max-w-md grid-cols-1 gap-2 sm:grid-cols-2") do
        render Campbooks::ConnectProviderCard.new(provider: :google, compact: true)
        render Campbooks::ConnectProviderCard.new(provider: :zoho, compact: true)
        # Microsoft is gated end-to-end (Features.microsoft?) until the Entra app is wired up.
        if helpers.microsoft_enabled?
          render Campbooks::ConnectProviderCard.new(provider: :microsoft, compact: true)
        end
      end
    end

    def render_dismiss(item)
      raw(safe(helpers.button_to(
        "✕", helpers.dismiss_setup_path(key: item[:key]), method: :post,
        class: "text-muted-foreground/50 hover:text-muted-foreground rounded p-1.5 text-sm leading-none cursor-pointer border-0 bg-transparent",
        form: { class: "shrink-0", title: t(".dismiss_title") }
      )))
    end

    def render_footer
      div(class: "flex items-center justify-between gap-3 px-4 py-2.5") do
        p(class: "text-xs text-muted-foreground") do
          overflow_count.positive? ? t(".more_in_settings", count: overflow_count) : t(".settings_hint")
        end
        # Native submit (turbo: false): snooze redirects to root, which Turbo would
        # otherwise re-render with the hub still showing.
        raw(safe(helpers.button_to(
          t(".leave_for_now"), helpers.snooze_onboarding_path, method: :post,
          class: "text-xs font-medium text-muted-foreground hover:text-foreground cursor-pointer border-0 bg-transparent whitespace-nowrap",
          form: { class: "inline shrink-0", data: { turbo: false } }
        )))
      end
    end

    def chevron_svg
      %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="h-4 w-4"><path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7"/></svg>)
    end
  end
end
