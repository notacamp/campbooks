# frozen_string_literal: true

module Campbooks
  # The first-run product walkthrough: a full-screen, multi-scene guided tour that
  # teaches the core loop with fake data and asks the user to *do* each task, not
  # just read about it. It progresses from the simplest idea (Scout surfaces what
  # matters) to richer ones (Skim a pile, capture a deadline, ask Scout), each
  # scene a "why + how + now you try".
  #
  # Self-contained and sandboxed: every interaction is handled client-side by the
  # `product-tour` Stimulus controller (mounted on <body>) — nothing touches the
  # user's real workspace, so it works before any inbox is connected and is safe
  # to replay. On finish/skip it POSTs the one-time tour flag (User#dismiss_tour!,
  # key "product_tour") so it greets the user only once; a "Take the tour" entry
  # and `?tour=1` re-open it on demand.
  #
  # Rendered once in the app layout (hidden until opened). The mock surfaces reuse
  # the real components (SkimCard, ScoutAvatar, Badge) so the tutorial looks exactly
  # like the app. Teaching copy is i18n'd; the sample email content (brand names,
  # subjects) stays as realistic fixed strings, like the existing skim demos.
  class ProductTour < Campbooks::Base
    # @param autostart [Boolean] open automatically on connect (first run on home)
    # @param connect_path [String, nil] where "Connect your inbox" sends the user;
    #   nil when the connect affordance sits right behind the overlay (the welcome
    #   screen) — the finish CTA then simply closes the tour.
    def initialize(autostart: false, connect_path: "/email_messages?inbox_settings=accounts", **attrs)
      @autostart = autostart
      @connect_path = connect_path
      @attrs = attrs
    end

    # Scene order, basics → complex. Used for the progress bar count and to keep the
    # controller's step label in sync without hardcoding a number.
    SCENES = %i[welcome feed skim reminder scout finish].freeze

    def view_template
      div(
        class: "hidden fixed inset-0 z-[70] flex-col bg-background",
        role: "dialog", aria_modal: "true", aria_label: t(".aria_label"),
        tabindex: "-1",
        data: {
          product_tour_target: "panel",
          tour_autostart: @autostart.to_s,
          action: "keydown->product-tour#onKeydown"
        },
        **@attrs
      ) do
        header
        div(class: "relative flex-1 overflow-y-auto") do
          # Short scenes settle at the optical middle; tall ones (skim) scroll.
          div(class: "flex min-h-full flex-col justify-center") do
            div(class: "mx-auto w-full max-w-xl px-5 py-8 sm:py-10") do
              scene_welcome
              scene_feed
              scene_skim
              scene_reminder
              scene_scout
              scene_finish
            end
          end
        end
        footer
      end
    end

    private

    # ── Chrome ──────────────────────────────────────────────────────────────

    def header
      div(class: "grid grid-cols-[1fr_auto_1fr] items-center gap-4 px-5 py-4") do
        # Live step announcement for assistive tech; the dots carry it visually.
        span(
          class: "sr-only",
          aria_live: "polite",
          data: { product_tour_target: "stepLabel", tmpl: t(".step", current: "{current}", total: "{total}") }
        ) { t(".step", current: 1, total: SCENES.size) }
        # Scene dots, centered: ember pill = where you are, ember dot = done,
        # bordered dot = still ahead.
        div(class: "col-start-2 flex items-center gap-2", aria_hidden: "true") do
          SCENES.size.times do |i|
            span(
              class: class_names(
                "h-2 rounded-full transition-all duration-300",
                i.zero? ? "w-5 bg-ember-gradient" : "w-2 border-[1.5px] border-border"
              ),
              data: { product_tour_target: "dot" }
            )
          end
        end
        button(
          type: "button",
          class: "col-start-3 justify-self-end rounded-lg px-2.5 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground",
          data: { action: "click->product-tour#skip" }
        ) { t(".skip") }
      end
    end

    def footer
      div(
        class: "flex items-center justify-between gap-3 border-t border-border px-5 py-3.5",
        data: { product_tour_target: "footer" }
      ) do
        button(
          type: "button",
          class: "rounded-xl px-4 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-0",
          data: { product_tour_target: "back", action: "click->product-tour#back" }
        ) { t(".back") }
        # Gated scenes disable Next until the task is done; the controller toggles it
        # and swaps the label to a finish CTA on the last scene before this footer
        # is hidden in favour of the finish buttons.
        button(
          type: "button",
          class: "inline-flex items-center gap-1.5 rounded-xl bg-ember-gradient px-5 py-2 text-sm font-semibold text-white shadow-ember transition-all duration-150 active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-40 disabled:shadow-none",
          data: { product_tour_target: "next", action: "click->product-tour#next" }
        ) do
          span(data: { product_tour_target: "nextLabel" }) { t(".next") }
          raw(safe(arrow_svg))
        end
      end
    end

    # ── Scene scaffolding ─────────────────────────────────────────────────────

    # One scene panel. `gated` scenes won't let the user advance until the task is
    # done; the controller reveals `done_cue` and enables Next on completion.
    def scene(key, gated: false, skim: false, &block)
      div(
        class: "hidden",
        data: {
          product_tour_target: "scene",
          tour_scene: key,
          tour_gated: gated.to_s,
          tour_skim: skim.to_s
        },
        &block
      )
    end

    # The teaching block atop a scene: a small kicker, the headline, the "why",
    # and (for interactive scenes) the "now you try" task line.
    def coach(eyebrow:, title:, why:, task: nil)
      div(class: "text-left") do
        span(class: "text-xs font-semibold uppercase tracking-[0.14em] text-ember") { eyebrow }
        h2(class: "mt-2 text-2xl font-bold leading-tight tracking-tight text-foreground text-balance") { title }
        p(class: "mt-2.5 text-[15px] leading-relaxed text-muted-foreground") { why }
        if task
          div(class: "mt-4 flex items-start gap-2.5 rounded-xl border border-border bg-muted/40 px-3.5 py-3") do
            span(class: "mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-ember-gradient text-white") do
              raw(safe(tap_svg))
            end
            p(class: "text-sm font-medium leading-snug text-foreground") { task }
          end
        end
      end
    end

    # Hidden success line the controller reveals once the scene's task is done.
    def done_cue(text)
      div(
        class: "mt-4 hidden items-start gap-2 rounded-xl border border-green-200 bg-green-50 px-3.5 py-3 text-sm font-medium text-green-800 dark:border-green-500/30 dark:bg-green-500/10 dark:text-green-300",
        data: { tour_done_cue: true, tour_flex: "true" }
      ) do
        raw(safe(check_svg))
        span { text }
      end
    end

    # ── Scene 1 · Welcome ─────────────────────────────────────────────────────

    def scene_welcome
      scene(:welcome) do
        div(class: "flex flex-col items-center pt-6 text-center") do
          render Campbooks::ScoutAvatar.new(size: :xl)
          h2(class: "mt-5 text-3xl font-bold tracking-tight text-foreground text-balance") { t(".s1_title") }
          p(class: "mt-3 max-w-md text-[15px] leading-relaxed text-muted-foreground") { t(".s1_body") }
          ul(class: "mx-auto mt-7 w-full max-w-xs space-y-2.5 text-left") do
            [ t(".s1_point_surface"), t(".s1_point_skim"), t(".s1_point_remind"), t(".s1_point_ask") ].each do |point|
              li(class: "flex items-start gap-2.5 text-sm text-foreground") do
                span(class: "mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-ember/15 text-ember") do
                  raw(safe(spark_svg))
                end
                span(class: "leading-snug") { point }
              end
            end
          end
        end
      end
    end

    # ── Scene 2 · The feed ────────────────────────────────────────────────────

    def scene_feed
      scene(:feed, gated: true) do
        coach(
          eyebrow: t(".s2_eyebrow"), title: t(".s2_title"),
          why: t(".s2_why"), task: t(".s2_task")
        )
        div(class: "mt-5 overflow-hidden rounded-2xl border border-border bg-card") do
          # The one item Scout surfaced — tapping it reveals the gist (no need to
          # open the email). Mirrors the home feed's attention card.
          button(
            type: "button",
            class: "block w-full px-4 py-4 text-left transition-colors hover:bg-muted/50 animate-tour-invite",
            data: { action: "click->product-tour#completeTask", tour_reveal: "#tour-feed-summary", tour_invite: "animate-tour-invite" }
          ) do
            div(class: "flex items-center gap-2") do
              span(class: "h-1.5 w-1.5 rounded-full", style: "background-color: var(--ember-solid)")
              span(class: "text-[11px] font-semibold uppercase tracking-[0.1em] text-ember") { t(".s2_badge") }
            end
            div(class: "mt-1.5 flex items-baseline justify-between gap-3") do
              span(class: "truncate text-sm font-semibold text-foreground") { "Northwind Bank" }
              span(class: "shrink-0 text-xs text-muted-foreground tabular-nums") { "09:14" }
            end
            p(class: "mt-0.5 truncate text-sm text-muted-foreground") { "Your transfer needs confirmation" }
            # Scout's note — an entity contributing, with a face and a name,
            # on the same ember glass it wears across the app.
            div(
              id: "tour-feed-summary",
              class: "scout-glass mt-3 hidden rounded-2xl px-3.5 py-3"
            ) do
              div(class: "flex items-center gap-2") do
                render Campbooks::ScoutAvatar.new(size: :xs)
                span(class: "text-[13px] font-semibold text-foreground") { "Scout" }
                span(class: "rounded-md border border-border bg-background/60 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground") { t(".ai_tag") }
              end
              p(class: "mt-2 text-sm leading-relaxed text-foreground") { t(".s2_summary") }
            end
          end
          # An ambient, low-priority row Scout kept out of the way — the contrast.
          div(class: "border-t border-border px-4 py-3.5 opacity-60") do
            div(class: "flex items-baseline justify-between gap-3") do
              span(class: "truncate text-sm font-medium text-foreground") { "LinkedIn" }
              span(class: "shrink-0 text-xs text-muted-foreground tabular-nums") { "08:30" }
            end
            p(class: "mt-0.5 truncate text-sm text-muted-foreground") { "5 people viewed your profile" }
          end
        end
        done_cue(t(".s2_feedback"))
      end
    end

    # ── Scene 3 · Skim ─────────────────────────────────────────────────────────

    # Realistic skim cards (samples, not live emails, so nothing tries to lazy-load
    # a body). One per "stack"; the controller plays them like Stories. Order
    # teaches the lesson: archive the noise, keep what matters.
    SKIM_CARDS = [
      {
        category: :notifications, title: "47 LinkedIn notifications", count: 47, bucket_label: "Today",
        summary: "Profile views and updates — nothing here needs a reply.",
        samples: [ "5 people viewed your profile", "Jordan started a new role", "3 jobs you may be interested in" ]
      },
      {
        category: :important, title: "Northwind Bank — verification code", count: 1, bucket_label: "Today",
        priority_suggested: true,
        summary: "A one-time code to confirm your transfer. Expires in 10 minutes.",
        samples: []
      },
      {
        category: :personal, title: "3 messages from Sarah", count: 3, bucket_label: "Yesterday",
        summary: "She's asking about lunch and shared a few photos to look at.",
        samples: [ "Lunch on Thursday?", "Photos from the weekend", "Did you see this?" ]
      }
    ].freeze

    def scene_skim
      scene(:skim, gated: true, skim: true) do
        coach(
          eyebrow: t(".s3_eyebrow"), title: t(".s3_title"),
          why: t(".s3_why"), task: t(".s3_task")
        )
        div(class: "mt-3 flex items-center justify-between") do
          span(
            class: "text-xs font-medium text-muted-foreground",
            data: { tour_skim_progress: true, tmpl: t(".s3_progress", done: "{done}", total: "{total}") }
          ) do
            t(".s3_progress", done: 0, total: SKIM_CARDS.size)
          end
        end
        # Stacked cards: the controller shows one at a time and advances on any
        # keep/archive (or other) action, played as Stories. Clicks on the cards'
        # built-in [data-skim-action] buttons bubble to product-tour#skimAct.
        # Grid-stack the cards (all in one cell) so a card flying out doesn't shove
        # the next one's layout — the outgoing and incoming overlap. The controller
        # (product-tour#skimAct) shows one at a time and animates the hand-off.
        div(class: "relative mt-2 grid", data: { action: "click->product-tour#skimAct" }) do
          SKIM_CARDS.each_with_index do |card, i|
            div(
              class: class_names("[grid-area:1/1] will-change-transform", i.zero? ? "" : "hidden"),
              data: { tour_skim_card: true }
            ) do
              render Campbooks::SkimCard.new(show_progress: false, **card)
            end
          end
        end
        done_cue(t(".s3_feedback"))
      end
    end

    # ── Scene 4 · Reminders ─────────────────────────────────────────────────────

    def scene_reminder
      scene(:reminder, gated: true) do
        coach(
          eyebrow: t(".s4_eyebrow"), title: t(".s4_title"),
          why: t(".s4_why"), task: t(".s4_task")
        )
        div(class: "mt-5 rounded-2xl border border-border bg-card p-4") do
          div(class: "flex items-baseline justify-between gap-3") do
            span(class: "truncate text-sm font-semibold text-foreground") { "Acme Supply Co." }
            span(class: "shrink-0 text-xs text-muted-foreground tabular-nums") { "Mon" }
          end
          p(class: "mt-0.5 text-sm text-muted-foreground") { "Invoice #4021 — $248.00" }
          # Scout's extracted commitment, shown like an inbox AI cue.
          div(class: "mt-3 flex items-center gap-2 rounded-xl bg-amber-50 px-3 py-2.5 text-amber-800 dark:bg-amber-500/10 dark:text-amber-300") do
            raw(safe(clock_svg))
            span(class: "text-sm font-medium") { t(".s4_extracted") }
          end
          div(class: "mt-3.5 flex items-center gap-2") do
            button(
              type: "button",
              class: "inline-flex items-center gap-1.5 rounded-lg bg-primary px-3.5 py-2 text-sm font-semibold text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50 animate-tour-breathe",
              data: { action: "click->product-tour#completeTask", tour_reveal: "#tour-reminder-chip", tour_consume: "true", tour_invite: "animate-tour-breathe" }
            ) do
              raw(safe(bell_svg))
              span { t(".s4_remind_cta") }
            end
            span(class: "text-xs text-muted-foreground") { t(".s4_dismiss") }
          end
          # Revealed once "Remind me" is tapped — the resulting reminder chip.
          div(
            id: "tour-reminder-chip",
            class: "mt-3 hidden items-center gap-2 rounded-xl border border-accent-200 bg-accent-50 px-3.5 py-2.5 dark:border-accent-500/30 dark:bg-accent-500/10",
            data: { tour_flex: "true" }
          ) do
            span(class: "flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-accent-600 text-white") { raw(safe(bell_svg)) }
            div(class: "min-w-0") do
              p(class: "truncate text-sm font-semibold text-accent-800 dark:text-accent-200") { t(".s4_reminder_title") }
              p(class: "truncate text-xs text-accent-700/80 dark:text-accent-300/80") { t(".s4_reminder_when") }
            end
          end
        end
        done_cue(t(".s4_feedback"))
      end
    end

    # ── Scene 5 · Scout ─────────────────────────────────────────────────────────

    def scene_scout
      scene(:scout, gated: true) do
        coach(
          eyebrow: t(".s5_eyebrow"), title: t(".s5_title"),
          why: t(".s5_why"), task: t(".s5_task")
        )
        div(class: "mt-5 rounded-2xl border border-border bg-card p-4") do
          # Scout's opener.
          div(class: "flex items-start gap-2.5") do
            render Campbooks::ScoutAvatar.new(size: :sm, class: "mt-0.5")
            div(class: "rounded-2xl rounded-tl-sm bg-muted px-3.5 py-2.5 text-sm leading-relaxed text-foreground") { t(".s5_greeting") }
          end
          # The suggested prompt — tapping it "asks" Scout and reveals the reply.
          div(class: "mt-3 flex justify-end") do
            button(
              type: "button",
              class: "rounded-2xl rounded-tr-sm border border-accent-300 bg-accent-50 px-3.5 py-2.5 text-left text-sm font-medium text-accent-800 transition-colors hover:bg-accent-100 disabled:opacity-60 animate-tour-breathe dark:border-accent-500/40 dark:bg-accent-500/10 dark:text-accent-200",
              data: { action: "click->product-tour#completeTask", tour_reveal: "#tour-scout-reply", tour_consume: "true", tour_invite: "animate-tour-breathe" }
            ) { t(".s5_prompt") }
          end
          # Scout's scripted answer.
          div(id: "tour-scout-reply", class: "mt-3 hidden items-start gap-2.5", data: { tour_flex: "true" }) do
            render Campbooks::ScoutAvatar.new(size: :sm, class: "mt-0.5")
            div(class: "rounded-2xl rounded-tl-sm bg-muted px-3.5 py-2.5 text-sm leading-relaxed text-foreground") do
              p { t(".s5_reply_intro") }
              ul(class: "mt-1.5 space-y-1") do
                [ t(".s5_reply_item_1"), t(".s5_reply_item_2") ].each do |line|
                  li(class: "flex items-start gap-1.5") do
                    span(class: "mt-1.5 h-1 w-1 shrink-0 rounded-full bg-muted-foreground/50")
                    span { line }
                  end
                end
              end
            end
          end
        end
        done_cue(t(".s5_feedback"))
      end
    end

    # ── Scene 6 · Finish ─────────────────────────────────────────────────────────

    def scene_finish
      scene(:finish) do
        div(class: "flex flex-col items-center pt-6 text-center") do
          # The one indulgent moment: the ember mark pops in under an expanding
          # glow ring when the scene enters (both are one-shot, reduced-motion safe).
          div(class: "relative") do
            span(
              class: "absolute -inset-3 rounded-full tour-finale-ring",
              style: "background: var(--ember); filter: blur(16px);",
              aria_hidden: "true"
            )
            span(class: "relative flex h-16 w-16 items-center justify-center rounded-2xl bg-ember-gradient text-white shadow-ember animate-sync-done-pop") do
              raw(safe(spark_svg(size: "h-8 w-8")))
            end
          end
          h2(class: "mt-6 text-3xl font-bold tracking-tight text-foreground text-balance") { t(".s6_title") }
          p(class: "mt-3 max-w-md text-[15px] leading-relaxed text-muted-foreground") { t(".s6_body") }
          div(class: "mt-8 flex w-full max-w-xs flex-col gap-2.5") do
            button(
              type: "button",
              class: "inline-flex w-full items-center justify-center gap-2 rounded-xl bg-ember-gradient px-5 py-3 text-sm font-semibold text-white shadow-ember transition-transform duration-150 active:scale-[0.98]",
              data: { action: "click->product-tour#finishConnect", tour_connect_path: @connect_path }.compact
            ) { t(".s6_cta_connect") }
            button(
              type: "button",
              class: "w-full rounded-xl px-5 py-3 text-sm font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground",
              data: { action: "click->product-tour#skip" }
            ) { t(".s6_cta_explore") }
          end
        end
      end
    end

    # ── Icons ─────────────────────────────────────────────────────────────────

    def arrow_svg
      %(<svg viewBox="0 0 20 20" fill="currentColor" class="h-4 w-4" aria-hidden="true"><path fill-rule="evenodd" d="M3 10a.75.75 0 01.75-.75h8.69L9.22 6.03a.75.75 0 111.06-1.06l4.5 4.5a.75.75 0 010 1.06l-4.5 4.5a.75.75 0 11-1.06-1.06l3.22-3.22H3.75A.75.75 0 013 10z" clip-rule="evenodd"/></svg>)
    end

    def check_svg
      %(<svg viewBox="0 0 20 20" fill="currentColor" class="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true"><path fill-rule="evenodd" d="M16.704 5.29a1 1 0 010 1.42l-7.5 7.5a1 1 0 01-1.42 0l-3.5-3.5a1 1 0 011.42-1.42l2.79 2.79 6.79-6.79a1 1 0 011.42 0z" clip-rule="evenodd"/></svg>)
    end

    def tap_svg
      %(<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M15.042 21.672 13.684 16.6m0 0-2.51 2.225.569-9.47 5.227 7.917-3.286-.672ZM12 2.25V4.5m5.834.166-1.591 1.591M20.25 10.5H18M7.757 14.743l-1.59 1.59M6 10.5H3.75m4.007-4.243-1.59-1.59"/></svg>)
    end

    def spark_svg(size: "h-3.5 w-3.5")
      %(<svg viewBox="0 0 24 24" fill="currentColor" class="#{size}" aria-hidden="true"><path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/></svg>)
    end

    def clock_svg
      %(<svg class="h-4 w-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.8" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 6v6l4 2m6-2a10 10 0 11-20 0 10 10 0 0120 0z"/></svg>)
    end

    def bell_svg
      %(<svg class="h-4 w-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.8" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M14.857 17.082a23.8 23.8 0 005.454-1.31A8.97 8.97 0 0118 9.75V9A6 6 0 006 9v.75a8.97 8.97 0 01-2.312 6.022c1.733.64 3.56 1.085 5.455 1.31m5.714 0a24 24 0 01-5.714 0m5.714 0a3 3 0 11-5.714 0"/></svg>)
    end
  end
end
