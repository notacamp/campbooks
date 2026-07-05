# frozen_string_literal: true

module Campbooks
  module Digests
    # One row on the /digests index page. Shows name, enabled badge, cadence line,
    # source chips, an inline enabled toggle (auto-submits a PATCH), edit link,
    # and a delete button with a turbo-confirm dialog.
    class DigestRow < Campbooks::Base
      def initialize(digest:)
        @digest = digest
      end

      def view_template
        div(
          id: helpers.dom_id(@digest),
          class: "flex flex-col gap-4 rounded-2xl border border-border bg-card px-5 py-4 sm:flex-row sm:items-center sm:justify-between"
        ) do
          left_col
          right_actions
        end
      end

      private

      attr_reader :digest

      def left_col
        div(class: "min-w-0 flex-1") do
          div(class: "flex flex-wrap items-center gap-2") do
            a(
              href: helpers.digest_path(digest),
              class: "truncate text-base font-semibold text-foreground hover:underline"
            ) { digest.name }
            status_badge
          end
          p(class: "mt-1 text-[13px] text-muted-foreground") { cadence_line }
          source_chips
        end
      end

      def status_badge
        if digest.enabled?
          span(class: "inline-flex items-center rounded-full bg-green-100 px-2 py-0.5 text-[10px] font-semibold text-green-700 dark:bg-green-500/15 dark:text-green-300") { t(".enabled") }
        else
          span(class: "inline-flex items-center rounded-full bg-muted px-2 py-0.5 text-[10px] font-semibold text-muted-foreground") { t(".disabled") }
        end
      end

      def cadence_line
        freq = helpers.t("digests.frequencies.#{digest.frequency}")
        "#{freq} · #{t(".next_run", time: helpers.l(digest.next_run_at, format: :short))}"
      end

      def source_chips
        return if digest.sources.empty?

        div(class: "mt-2 flex flex-wrap gap-1.5") do
          digest.sources.each do |src|
            type = src["type"].to_s
            span(class: "inline-flex items-center rounded-md bg-muted px-2 py-0.5 text-[11px] font-medium text-foreground/70") do
              plain helpers.t("digests.sections.#{type}", default: type.humanize)
            end
          end
        end
      end

      def right_actions
        div(class: "flex flex-shrink-0 items-center gap-2") do
          enabled_toggle
          edit_link
          delete_form
        end
      end

      # Partial update: submits ONLY digest[enabled] (flipped). The controller
      # leaves config and the other flags untouched for source-less submits.
      def enabled_toggle
        form(
          action: helpers.digest_path(digest),
          method: :post,
          data: { turbo_stream: true }
        ) do
          input(type: "hidden", name: "_method", value: "patch")
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          input(type: "hidden", name: "digest[enabled]", value: digest.enabled? ? "0" : "1")
          button(
            type: "submit",
            class: "group flex items-center gap-1.5 rounded-lg p-1.5 text-[12px] text-muted-foreground transition-colors hover:bg-muted",
            title: digest.enabled? ? t(".disable") : t(".enable"),
            aria_label: digest.enabled? ? t(".disable") : t(".enable")
          ) do
            # Visual state only — pointer-events-none so the click always hits
            # the submit button (a live checkbox inside a button is invalid).
            div(class: "pointer-events-none", aria_hidden: "true") do
              render Campbooks::Toggle.new(
                name: "digest[enabled_display]",
                checked: digest.enabled?,
                disabled: false,
                tabindex: "-1"
              )
            end
          end
        end
      end

      def edit_link
        a(
          href: helpers.edit_digest_path(digest),
          class: "rounded-lg p-2 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground",
          title: helpers.t("shared.actions.edit")
        ) do
          raw safe('<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>')
        end
      end

      def delete_form
        form(
          action: helpers.digest_path(digest),
          method: :post,
          data: { turbo_stream: true, turbo_confirm: t(".confirm_delete") }
        ) do
          input(type: "hidden", name: "_method", value: "delete")
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(
            type: "submit",
            class: "rounded-lg p-2 text-muted-foreground transition-colors hover:bg-red-50 hover:text-red-600 dark:hover:bg-red-500/10",
            title: helpers.t("shared.actions.delete")
          ) do
            raw safe('<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6"/><path d="M10 11v6M14 11v6"/><path d="M9 6V4a1 1 0 011-1h4a1 1 0 011 1v2"/></svg>')
          end
        end
      end
    end
  end
end
