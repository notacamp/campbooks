# frozen_string_literal: true

module Campbooks
  # The "Scout is reading your inbox" stage: what a brand-new user watches while
  # their first scan runs, instead of an empty feed. Scout sits under a breathing
  # ember halo over three live counters (found / sorted / needs you) that tick up
  # as mail lands; when the scan completes the halo gives way to a check, the
  # headline flips to the payoff, and one CTA reveals the sorted feed.
  #
  # Server renders the initial numbers; the `first-sync` Stimulus controller
  # polls `status_url` and owns every transition (scanning → done / empty /
  # error). All copy lives here as data-template strings so the JS stays
  # translation-free.
  #
  # Optional persona card: when `persona_url` is given, a compact persona
  # question appears below the counters while the scan runs. Submitting applies
  # the setup templates mid-scan via Turbo Stream. Pass `skip_url` to turn the
  # escape-hatch link into a POST that sets a session flag so home won't
  # re-trap the user in this stage.
  class FirstSyncStage < Campbooks::Base
    # @param status [Hash] {state:, found:, sorted:, needs_you:} — initial values
    # @param status_url [String] JSON endpoint the controller polls
    # @param inbox_path [String] escape hatch ("watch the inbox fill instead")
    # @param feed_path [String] where the done CTA lands (home)
    # @param persona_url [String, nil] POST endpoint for persona selection (nil = feature off)
    # @param skip_url [String, nil] POST endpoint for skipping the stage (nil = link fallback)
    def initialize(status:, status_url:, inbox_path: "/email_messages", feed_path: "/", persona_url: nil, skip_url: nil, **attrs)
      @status = status
      @status_url = status_url
      @inbox_path = inbox_path
      @feed_path = feed_path
      @persona_url = persona_url
      @skip_url = skip_url
      @attrs = attrs
    end

    def view_template
      custom_class = @attrs.delete(:class)
      div(
        class: class_names("flex flex-col items-center text-center", custom_class),
        data: {
          controller: "first-sync",
          first_sync_url_value: @status_url,
          first_sync_state_value: @status[:state].to_s,
          first_sync_feed_path_value: @feed_path,
          first_sync_reveal_delay_value: 12_000
        },
        **@attrs
      ) do
        stage_mark
        headline
        counters
        persona_card if @persona_url
        done_cta
        error_note
        escape_hatch
      end
    end

    private

    # Scout under the breathing halo; swapped for the check when the scan lands.
    def stage_mark
      div(class: "relative animate-stage-in-hero") do
        span(
          class: "absolute -inset-3 rounded-full animate-sync-halo",
          style: "background: var(--ember); filter: blur(14px);",
          aria_hidden: "true",
          data: { first_sync_target: "halo" }
        )
        div(class: "relative") { render Campbooks::ScoutAvatar.new(size: :xl) }
        span(
          class: "absolute -bottom-1 -right-1 hidden h-6 w-6 items-center justify-center rounded-full bg-green-600 text-white ring-2 ring-background",
          data: { first_sync_target: "check" }
        ) { raw(safe(check_svg)) }
      end
    end

    def headline
      h1(
        class: "mt-6 text-[1.7rem] font-semibold leading-tight tracking-[-0.02em] text-foreground text-balance animate-stage-in",
        style: "--stage-delay: .05s",
        data: {
          first_sync_target: "title",
          tmpl_scanning: t(".title_scanning"),
          tmpl_done: t(".title_done"),
          tmpl_empty: t(".title_empty"),
          tmpl_error: t(".title_error")
        }
      ) { t(".title_scanning") }
      p(
        class: "mt-2.5 max-w-sm text-[15px] leading-relaxed text-muted-foreground text-pretty animate-stage-in",
        style: "--stage-delay: .1s",
        data: {
          first_sync_target: "subtitle",
          tmpl_waiting: t(".subtitle_waiting"),
          tmpl_scanning: t(".subtitle_scanning"),
          tmpl_done: t(".subtitle_done", needs_you: "{needs_you}"),
          tmpl_done_calm: t(".subtitle_done_calm"),
          tmpl_empty: t(".subtitle_empty"),
          tmpl_error: t(".subtitle_error")
        }
      ) { t(".subtitle_waiting") }
    end

    def counters
      div(class: "mt-9 flex items-start justify-center gap-9 sm:gap-12 animate-stage-in", style: "--stage-delay: .16s") do
        counter(:found, t(".counter_found"))
        counter(:sorted, t(".counter_sorted"))
        counter(:needs_you, t(".counter_needs_you"), ember: true)
      end
    end

    def counter(key, label, ember: false)
      div(class: "min-w-[4.5rem]") do
        p(
          class: "text-[2rem] font-semibold leading-none tabular-nums tracking-tight text-foreground",
          data: { first_sync_target: key.to_s.camelize(:lower) }
        ) { @status[key].to_i.to_s }
        p(class: "mt-1.5 flex items-center justify-center gap-1.5 text-xs font-medium text-muted-foreground") do
          if ember
            span(class: "h-1.5 w-1.5 rounded-full", style: "background-color: var(--ember-solid)", aria_hidden: "true")
          end
          plain(label)
        end
      end
    end

    # Compact persona question shown while the scan runs. NOT a Stimulus target —
    # Turbo replace owns the entire wrapper when the user submits. The polling
    # controller hides it when the scan finishes via document.getElementById.
    def persona_card
      state = @status[:state]&.to_sym
      return if %i[done empty error].include?(state)

      div(
        id: "first-sync-persona",
        class: "mt-9 w-full max-w-sm rounded-2xl border border-border bg-card p-4 text-left animate-stage-in",
        style: "--stage-delay: .22s"
      ) do
        h2(class: "text-[13px] font-semibold text-foreground") { t(".persona_heading") }
        p(class: "mt-0.5 text-xs text-muted-foreground") { t(".persona_subheading") }

        form(action: @persona_url, method: "post", class: "mt-3", data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)

          # 2-col grid of compact icon + name chips; collapses to 1 col at narrow widths
          div(class: "grid grid-cols-1 gap-1.5 sm:grid-cols-2") do
            Onboarding::Templates.all.each do |tpl|
              key = tpl[:key]
              label(
                class: "flex cursor-pointer items-center gap-2 rounded-xl border border-border bg-background p-2.5 text-left transition-all hover:border-accent-300 hover:bg-accent-50/40 has-[:checked]:border-accent-500 has-[:checked]:bg-accent-50/60 dark:hover:bg-accent-900/10 dark:has-[:checked]:bg-accent-900/20"
              ) do
                input(type: "checkbox", name: "template_keys[]", value: key, class: "sr-only")
                span(class: "flex h-5 w-5 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground") do
                  raw safe(%(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" class="h-3 w-3" aria-hidden="true"><path d="#{tpl[:icon]}"/></svg>))
                end
                span(class: "min-w-0 flex-1 truncate text-xs font-medium text-foreground leading-snug") do
                  plain t("onboarding.steps.template.templates.#{key}.name")
                end
              end
            end
          end

          div(class: "mt-3 flex items-center gap-3") do
            button(
              type: "submit",
              class: "rounded-xl bg-primary px-4 py-1.5 text-xs font-semibold text-primary-foreground transition-colors hover:bg-primary/90 active:scale-[0.98]"
            ) { t(".persona_submit") }
          end
        end

        # Skip: separate form posting empty keys — returns the "exploring" confirmation
        form(action: @persona_url, method: "post", class: "mt-1.5 inline-block", data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit", class: "text-xs text-muted-foreground transition-colors hover:text-foreground focus:outline-none") do
            t(".persona_skip")
          end
        end
      end
    end

    def done_cta
      div(class: "mt-9 hidden w-full max-w-xs flex-col items-center gap-3", data: { first_sync_target: "doneCta" }) do
        button(
          type: "button",
          class: "inline-flex w-full items-center justify-center gap-2 rounded-xl bg-ember-gradient px-5 py-3 text-sm font-semibold text-white shadow-ember transition-transform duration-150 active:scale-[0.98]",
          data: { action: "click->first-sync#reveal" }
        ) { t(".cta_done") }
      end
    end

    def error_note
      div(
        class: "mt-9 hidden w-full max-w-sm items-center gap-3 rounded-2xl border border-amber-200 bg-amber-50 px-4 py-3 text-left dark:border-amber-500/30 dark:bg-amber-500/10",
        data: { first_sync_target: "errorNote" }
      ) do
        span(class: "flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-amber-100 text-amber-700 dark:bg-amber-500/20 dark:text-amber-300") do
          raw(safe(alert_svg))
        end
        div(class: "min-w-0 flex-1") do
          p(class: "text-sm font-semibold text-amber-900 dark:text-amber-200") { t(".error_title") }
          a(href: helpers.email_messages_path(inbox_settings: "accounts"), class: "text-[13px] font-medium text-amber-800 underline-offset-2 hover:underline dark:text-amber-300") do
            t(".error_cta")
          end
        end
      end
    end

    # Appears after a while for slow scans — the stage never traps anyone.
    # When skip_url is provided, posting skips out and prevents re-entrapment on
    # the next home visit. Falls back to a plain anchor when skip_url is nil.
    def escape_hatch
      if @skip_url
        # The FORM carries the Stimulus target so JS can show/hide the whole unit.
        form(
          action: @skip_url,
          method: "post",
          class: "mt-10 hidden",
          data: { first_sync_target: "escape" }
        ) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(
            type: "submit",
            class: "text-sm font-medium text-muted-foreground transition-colors hover:text-foreground focus:outline-none"
          ) { t(".escape") }
        end
      else
        a(
          href: @inbox_path,
          class: "mt-10 hidden text-sm font-medium text-muted-foreground transition-colors hover:text-foreground",
          data: { first_sync_target: "escape" }
        ) { t(".escape") }
      end
    end

    def check_svg
      %(<svg viewBox="0 0 20 20" fill="currentColor" class="h-3.5 w-3.5" aria-hidden="true"><path fill-rule="evenodd" d="M16.704 5.29a1 1 0 010 1.42l-7.5 7.5a1 1 0 01-1.42 0l-3.5-3.5a1 1 0 011.42-1.42l2.79 2.79 6.79-6.79a1 1 0 011.42 0z" clip-rule="evenodd"/></svg>)
    end

    def alert_svg
      %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="h-4 w-4" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z"/></svg>)
    end
  end
end
