# frozen_string_literal: true

module Campbooks
  # The "getting started" checklist that replaces the stacked setup banners: one
  # card showing progress and the remaining setup tasks, each with its own CTA.
  # Pure/data-driven so it previews without a workspace — the partial extracts
  # the data from SetupStatus.
  #
  # Renders inside the existing #setup_banner wrapper, so the other setup modals'
  # completion stream (turbo_stream.replace "setup_banner") keeps refreshing it.
  class SetupHub < Campbooks::Base
    # @param items [Array<Hash>] all setup tasks (SetupStatus::ITEMS shape)
    # @param incomplete_keys [Array<Symbol,String>] keys of still-incomplete tasks
    # @param dismissed_keys [Array<String>] keys the user dismissed
    # @param return_to [String] path to return to after a setup modal completes
    # @param collapsed [Boolean] start collapsed (used on the dense inbox)
    def initialize(items:, incomplete_keys:, dismissed_keys: [], return_to: "/", collapsed: false, **attrs)
      @items = items
      @incomplete_keys = incomplete_keys.map(&:to_sym)
      @dismissed_keys = dismissed_keys.map(&:to_s)
      @return_to = return_to
      @collapsed = collapsed
      @attrs = attrs
    end

    def view_template
      return if visible_items.empty?

      custom_class = @attrs.delete(:class)
      details(
        id: "setup_hub", open: !@collapsed,
        class: class_names(
          "group/hub block bg-card text-card-foreground rounded-2xl border border-border shadow-sm overflow-hidden",
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

    def visible_items
      @visible_items ||= @items.reject { |item| incomplete?(item) && dismissed?(item) }
    end

    def incomplete?(item)
      @incomplete_keys.include?(item[:key])
    end

    def dismissed?(item)
      @dismissed_keys.include?(item[:key].to_s)
    end

    def total
      @items.size
    end

    def completed_count
      total - @incomplete_keys.size
    end

    def percent
      total.zero? ? 0 : ((completed_count.to_f / total) * 100).round
    end

    def first_incomplete_key
      @first_incomplete_key ||= visible_items.find { |item| incomplete?(item) }&.dig(:key)
    end

    def render_summary
      summary(class: "list-none cursor-pointer select-none px-5 py-4 [&::-webkit-details-marker]:hidden") do
        div(class: "flex items-center gap-3.5") do
          span(class: "flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-accent-50 text-accent-600") do
            raw(safe(sparkle_svg))
          end
          div(class: "min-w-0 flex-1") do
            p(class: "text-base font-semibold tracking-tight text-foreground") { t(".heading") }
            p(class: "text-[13px] text-muted-foreground") { t(".progress", completed: completed_count, total: total) }
          end
          render Campbooks::Badge.new(variant: percent == 100 ? :success : :accent, size: :md) { "#{percent}%" }
          span(class: "text-muted-foreground transition-transform duration-200 group-open/hub:rotate-180") do
            raw(safe(chevron_svg))
          end
        end
        div(class: "mt-3.5 h-2 w-full overflow-hidden rounded-full bg-border") do
          div(class: "h-full rounded-full bg-accent-600 transition-all duration-500", style: "width: #{percent}%")
        end
      end
    end

    def render_rows
      ul(class: "divide-y divide-border") do
        visible_items.each { |item| render_row(item) }
      end
    end

    def render_row(item)
      done = !incomplete?(item)
      current = item[:key] == first_incomplete_key

      li(id: "setup_hub_row_#{item[:key]}", class: "flex items-start gap-3.5 px-5 py-4") do
        div(class: "mt-0.5 shrink-0") { render_status_icon(done: done, current: current) }
        div(class: "min-w-0 flex-1") do
          p(class: class_names("text-[15px] font-semibold", done ? "text-muted-foreground line-through" : "text-foreground")) do
            item[:message]
          end
          unless done
            p(class: "mt-1 text-[13px] leading-relaxed text-muted-foreground line-clamp-2") { item[:description] }
            div(class: "mt-3 flex flex-wrap items-center gap-2") { render_ctas(item) }
          end
        end
        render_dismiss(item) unless done
      end
    end

    def render_ctas(item)
      case item[:key]
      when :email_account then render_email_ctas
      when :document_types then render_ai_ctas("document_types", item)
      when :tags then render_ai_ctas("tags", item)
      else render_modal_cta(item)
      end
    end

    def render_modal_cta(item, step: item[:key])
      render Campbooks::Button.new(
        variant: :primary, size: :sm,
        data: { setup_modal_open: helpers.setup_path(step, return_to: @return_to) }
      ) { "#{item[:cta_text]} →" }
    end

    # AI-first for types/tags: lead with "Let AI help", keep manual entry as the
    # secondary option.
    def render_ai_ctas(kind, item)
      render Campbooks::Button.new(
        variant: :primary, size: :sm,
        data: { setup_modal_open: helpers.ai_setup_chat_path(kind: kind) }
      ) { t(".let_ai_help") }
      render Campbooks::Button.new(
        variant: :outline, size: :sm,
        data: { setup_modal_open: helpers.setup_path(item[:key], return_to: @return_to) }
      ) { t(".add_manually") }
    end

    def render_email_ctas
      raw(safe(provider_button(t(".connect_zoho"), "zoho", "bg-red-600 hover:bg-red-700")))
      raw(safe(provider_button(t(".connect_google"), "google", "bg-blue-600 hover:bg-blue-700")))
      # Microsoft is gated end-to-end (Features.microsoft?) until the Entra app is wired up.
      if helpers.microsoft_enabled?
        raw(safe(provider_button(t(".connect_microsoft"), "microsoft", "bg-accent-600 hover:bg-accent-700")))
      end
    end

    def provider_button(label, provider, color)
      helpers.button_to(
        label, helpers.email_accounts_path(provider: provider), method: :post,
        data: { turbo: false },
        class: "inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium text-white #{color} cursor-pointer border-0 transition-colors"
      )
    end

    def render_dismiss(item)
      raw(safe(helpers.button_to(
        "✕", helpers.dismiss_setup_path(key: item[:key]), method: :post,
        class: "text-muted-foreground/50 hover:text-muted-foreground rounded p-1.5 text-sm leading-none cursor-pointer border-0 bg-transparent",
        form: { class: "shrink-0", title: t(".dismiss_title") }
      )))
    end

    def render_footer
      return if visible_items.none? { |item| incomplete?(item) }

      div(class: "px-5 py-3 text-right") do
        # Native submit (turbo: false): snooze redirects to root, which Turbo would
        # otherwise re-render with the hub still showing. Mirrors OnboardingProgress.
        raw(safe(helpers.button_to(
          t(".leave_for_now"), helpers.snooze_onboarding_path, method: :post,
          class: "text-[13px] text-muted-foreground hover:text-foreground cursor-pointer border-0 bg-transparent",
          form: { class: "inline", data: { turbo: false } }
        )))
      end
    end

    def render_status_icon(done:, current:)
      if done
        span(class: "flex h-6 w-6 items-center justify-center rounded-full bg-green-100 text-green-600") do
          raw(safe(check_svg))
        end
      elsif current
        span(class: "flex h-6 w-6 items-center justify-center rounded-full bg-accent-100 text-accent-600") do
          raw(safe(arrow_svg))
        end
      else
        span(class: "block h-6 w-6 rounded-full border-2 border-border")
      end
    end

    def sparkle_svg
      %(<svg viewBox="0 0 24 24" fill="currentColor" class="h-5 w-5"><path fill-rule="evenodd" d="M9 4.5a.75.75 0 01.721.544l.813 2.846a3.75 3.75 0 002.576 2.576l2.846.813a.75.75 0 010 1.442l-2.846.813a3.75 3.75 0 00-2.576 2.576l-.813 2.846a.75.75 0 01-1.442 0l-.813-2.846a3.75 3.75 0 00-2.576-2.576l-2.846-.813a.75.75 0 010-1.442l2.846-.813A3.75 3.75 0 007.466 7.89l.813-2.846A.75.75 0 019 4.5z" clip-rule="evenodd"/></svg>)
    end

    def chevron_svg
      %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="h-5 w-5"><path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7"/></svg>)
    end

    def check_svg
      %(<svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4"><path fill-rule="evenodd" d="M16.704 5.29a1 1 0 010 1.42l-7.5 7.5a1 1 0 01-1.42 0l-3.5-3.5a1 1 0 011.42-1.42l2.79 2.79 6.79-6.79a1 1 0 011.42 0z" clip-rule="evenodd"/></svg>)
    end

    def arrow_svg
      %(<svg viewBox="0 0 20 20" fill="currentColor" class="h-3.5 w-3.5"><path fill-rule="evenodd" d="M3 10a.75.75 0 01.75-.75h8.69L9.22 6.03a.75.75 0 111.06-1.06l4.5 4.5a.75.75 0 010 1.06l-4.5 4.5a.75.75 0 11-1.06-1.06l3.22-3.22H3.75A.75.75 0 013 10z" clip-rule="evenodd"/></svg>)
    end
  end
end
