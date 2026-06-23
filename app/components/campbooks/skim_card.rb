# frozen_string_literal: true

module Campbooks
  # A cluster "story card": review a whole stack of similar emails as one unit.
  # The SkimBuilder collapses e.g. 47 CircleCI builds (or one conversation thread)
  # into a single card, so the user makes one decision instead of forty-seven.
  #
  # Time is shown up front (the card's latest arrival, and each row's), because
  # recency is Skim's backbone. Importance is only ever a *suggestion*: a pinned
  # card wears a "Priority" pill; a card the AI thinks is worth prioritising wears
  # a confirmable "Suggested" cue — never an asserted "this is important".
  class SkimCard < Campbooks::Base
    # Above this many stacks, individual progress dots would overflow — switch to
    # a slim proportional bar instead.
    MAX_DOTS = 12

    CARD_ACCENT = {
      personal:  "border-accent-300/60",
      important: "border-amber-300 dark:border-amber-500/40"
    }.freeze

    ICONS = {
      archive: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"/>',
      keep:    '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/>',
      allow:   '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>',
      deny:    '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M18.364 5.636l-12.728 12.728M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>',
      block:   '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M18.364 5.636l-12.728 12.728M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>',
      star:        '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11.48 3.5a.56.56 0 011.04 0l2.12 4.92 5.34.46c.49.04.69.66.31.98l-4.05 3.5 1.21 5.22c.11.48-.41.86-.83.6L12 17.27l-4.63 2.91c-.42.26-.94-.12-.83-.6l1.21-5.22-4.05-3.5c-.38-.32-.18-.94.31-.98l5.34-.46 2.12-4.92z"/>',
      star_filled: '<path d="M11.48 3.5a.56.56 0 011.04 0l2.12 4.92 5.34.46c.49.04.69.66.31.98l-4.05 3.5 1.21 5.22c.11.48-.41.86-.83.6L12 17.27l-4.63 2.91c-.42.26-.94-.12-.83-.6l1.21-5.22-4.05-3.5c-.38-.32-.18-.94.31-.98l5.34-.46 2.12-4.92z"/>',
      sparkle:     '<path d="M9 3a.5.5 0 01.47.33l1.1 3.1 3.1 1.1a.5.5 0 010 .94l-3.1 1.1-1.1 3.1a.5.5 0 01-.94 0l-1.1-3.1-3.1-1.1a.5.5 0 010-.94l3.1-1.1 1.1-3.1A.5.5 0 019 3zM17 13a.4.4 0 01.38.27l.62 1.73 1.73.62a.4.4 0 010 .76l-1.73.62-.62 1.73a.4.4 0 01-.76 0l-.62-1.73-1.73-.62a.4.4 0 010-.76l1.73-.62.62-1.73A.4.4 0 0117 13z"/>',
      dismiss:     '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>',
      clock:       '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z"/>'
    }.freeze

    # @param category [Symbol] triage category (CategoryChip::CATEGORIES) — a cue
    # @param title [String] cluster title
    # @param count [Integer] number of emails in the cluster
    # @param summary [String, nil] one-line "so what"
    # @param samples [Array<String>] representative subjects (collapsed peek)
    # @param emails [Array<Hash>] {id:, sender:, subject:, snippet:, received_at:}
    # @param latest_received_at [Time, nil] the cluster's most recent arrival
    # @param pinned [Boolean] already in the Priority lane
    # @param priority_suggested [Boolean] the AI suggests promoting this (confirmable)
    # @param scout_suggestion [Hash, nil] learned action from past decisions,
    #   { action: "archive"|"keep"|"promote", count:, total: }. Surfaces a Scout
    #   "you usually X these" line and makes that verb the primary button.
    # @param summary_digest [String, nil] cluster digest for a multi-email card — the
    #   DOM id Emails::SkimSummaryJob live-swaps the AI summary into when it's ready.
    # @param position/total [Integer, nil] story progress
    # @param show_progress [Boolean] render the built-in dots/bar
    # @param fill [Boolean] tall "story frame" layout
    def initialize(category:, title:, count:, summary: nil, samples: [], emails: [],
                   latest_received_at: nil, bucket_label: nil, pinned: false, priority_suggested: false,
                   scout_suggestion: nil, summary_digest: nil, follow_up_reason: nil, theme: nil, position: nil, total: nil, show_progress: true, fill: false, **attrs)
      @category = category
      @title = title
      @count = count
      @summary = summary
      @samples = samples || []
      @emails = emails || []
      @latest_received_at = latest_received_at
      @bucket_label = bucket_label
      @pinned = pinned
      @priority_suggested = priority_suggested
      @scout_suggestion = scout_suggestion
      @summary_digest = summary_digest
      @follow_up_reason = follow_up_reason
      @theme = theme&.to_sym
      @position = position
      @total = total
      @show_progress = show_progress
      @fill = fill
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      div(
        class: class_names(
          "w-full rounded-xl border bg-card shadow-sm",
          @fill ? "flex min-h-[26rem] max-h-[82vh] flex-col p-6 sm:min-h-[30rem]" : "max-w-sm p-5",
          accent_border,
          custom
        ),
        **@attrs
      ) do
        progress if @show_progress
        header
        h3(class: class_names("font-semibold text-foreground", @fill ? "mt-3 text-xl font-bold leading-tight text-balance sm:text-2xl" : "mt-3 text-base leading-snug")) { @title }
        render Campbooks::SkimSummary.new(text: @summary, digest: @summary_digest, fill: @fill) if @summary
        scout_line
        follow_up_note
        if @fill
          div(class: "mt-4 flex-1 overflow-y-auto overscroll-contain") { email_list }
        else
          email_list
        end
        actions
      end
    end

    private

    def accent_border
      return "border-accent-400/70" if @theme == :starred
      return "border-dashed border-muted-foreground/40" if @theme == :pending
      return "border-amber-300 dark:border-amber-500/50" if @pinned
      return "border-accent-400/60" if @priority_suggested

      CARD_ACCENT[@category] || "border-border"
    end

    # A single email shows its whole body inline (read it without a click); a
    # multi-email cluster shows the expandable list; previews fall back to samples.
    def email_list
      return single_email_body if @emails.size == 1 && @emails.first[:id]
      @emails.any? ? detail : peek
    end

    # The whole email, shown inline automatically (it's the only one). The body
    # loads lazily — only the visible card fetches it (skim-mode#loadCurrentBody),
    # keyed off data-skim-autoload so multi-email rows only load on expand. When the
    # stored body is empty we fall back to the email's summary (fallback=summary) so
    # a single-email card always reads as content, never just a "No preview" stub.
    def single_email_body
      em = @emails.first
      div(class: "mt-4 border-t border-border pt-3") do
        div(class: "flex items-center justify-between gap-2") do
          span(class: "min-w-0 truncate text-xs font-medium text-muted-foreground") { em[:sender].to_s }
          if em[:received_at]
            span(class: "flex-shrink-0 text-[11px] text-muted-foreground/80 tabular-nums") { helpers.thread_date_label(em[:received_at]) }
          end
        end
        raw(safe(%(<turbo-frame id="skim_body_#{em[:id]}" data-skim-autoload="true" data-skim-body-src="/skim/email/#{em[:id]}/content?fallback=summary" class="mt-2 block text-sm leading-relaxed text-foreground [&_a]:text-accent-600 [&_a]:underline [&_img]:max-w-full"><p class="text-xs text-muted-foreground">Loading…</p></turbo-frame>)))
        div(class: "mt-3 flex items-center gap-3 text-xs font-medium") do
          a(href: "/skim/email/#{em[:id]}", class: "inline-flex items-center gap-1 text-accent-600 hover:underline", data: { turbo_frame: "skim_email_card", action: "click->skim-mode#openEmail" }) do
            plain t(".reply")
            span(aria_hidden: "true") { "↗" }
          end
          a(href: "/email_messages/#{em[:id]}", class: "inline-flex items-center gap-1 text-muted-foreground hover:underline", data: { turbo_frame: "_top" }) do
            plain t(".open_in_full")
            span(aria_hidden: "true") { "↗" }
          end
        end
      end
    end

    def progress
      return unless @position && @total

      if @total > MAX_DOTS
        div(
          class: "mb-4 h-1 w-full overflow-hidden rounded-full bg-muted",
          role: "progressbar", aria_valuenow: @position, aria_valuemin: 1, aria_valuemax: @total,
          aria_label: t(".progress_label", position: @position, total: @total)
        ) do
          div(class: "h-full rounded-full bg-accent-500", style: "width: #{((@position.to_f / @total) * 100).round}%")
        end
      else
        div(class: "mb-4 flex items-center gap-1", aria_label: t(".progress_label", position: @position, total: @total)) do
          (1..@total).each do |i|
            div(class: class_names("h-1 rounded-full transition-all", i == @position ? "w-6 bg-accent-500" : "w-1.5 bg-muted"))
          end
        end
      end
    end

    # The theme lives in the viewer header; the card leads with WHEN it arrived
    # (the time bucket) since that's what distinguishes cards within a theme.
    def header
      div(class: "flex items-center justify-between gap-3") do
        div(class: "flex min-w-0 items-center gap-2") do
          bucket_pill
          status_pill
        end
        div(class: "flex flex-shrink-0 items-center gap-2") do
          time_label
          span(class: "text-xs text-muted-foreground tabular-nums", data: { skim_count: true }) { t(".emails", count: @count) }
        end
      end
    end

    def bucket_pill
      return unless @bucket_label

      span(class: "inline-flex flex-shrink-0 items-center rounded-full bg-muted px-2 py-0.5 text-[11px] font-semibold text-muted-foreground") { @bucket_label }
    end

    # Starred sender → a gold "Starred" pill (this mail is promoted). Pinned → a
    # solid "Priority" pill; AI-suggested → a confirmable "Suggested" cue.
    def status_pill
      if @theme == :follow_ups
        span(class: "inline-flex flex-shrink-0 items-center gap-1 rounded-full bg-amber-100 px-2 py-0.5 text-[11px] font-semibold text-amber-700 dark:bg-amber-500/15 dark:text-amber-300") do
          svg(class: "h-3 w-3", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[:clock])) }
          plain t(".waiting")
        end
      elsif @theme == :starred
        span(class: "inline-flex flex-shrink-0 items-center gap-1 rounded-full bg-accent-100 px-2 py-0.5 text-[11px] font-semibold text-accent-700 dark:bg-accent-500/15 dark:text-accent-300") do
          star_icon(filled: true, klass: "h-3 w-3")
          plain t(".starred")
        end
      elsif @pinned
        span(class: "inline-flex flex-shrink-0 items-center gap-1 rounded-full bg-amber-100 px-2 py-0.5 text-[11px] font-semibold text-amber-700 dark:bg-amber-500/15 dark:text-amber-300") do
          star_icon(filled: true, klass: "h-3 w-3")
          plain t(".priority")
        end
      elsif @priority_suggested
        span(class: "inline-flex flex-shrink-0 items-center gap-1 rounded-full bg-accent-100 px-2 py-0.5 text-[11px] font-semibold text-accent-700 dark:bg-accent-500/15 dark:text-accent-300") do
          star_icon(filled: false, klass: "h-3 w-3")
          plain t(".suggested")
        end
      end
    end

    def time_label
      return unless @latest_received_at

      span(class: "text-xs text-muted-foreground tabular-nums") { helpers.thread_date_label(@latest_received_at) }
    end

    # Fallback preview when a card carries no full email rows (previews / demo).
    def peek
      shown = @samples.first(3)
      return if shown.empty?

      div(class: "mt-4 space-y-1.5") do
        shown.each do |subject|
          div(class: "flex items-center gap-2 text-xs text-muted-foreground") do
            div(class: "h-1 w-1 flex-shrink-0 rounded-full bg-muted-foreground/40")
            span(class: "truncate") { subject }
          end
        end
        remaining = @count - shown.size
        div(class: "pl-3 text-xs text-muted-foreground/70") { t(".more", count: remaining) } if remaining.positive?
      end
    end

    # The cluster's emails. Each row expands IN PLACE (accordion) to reveal the
    # whole body inline — no stacked card, no "open in full" needed to read. The
    # body loads lazily on first expand. An optional checkbox (multi-email
    # clusters) drives partial Archive; Reply opens the composer over the stack.
    def detail
      return if @emails.empty?

      selectable = @emails.size > 1

      div(class: "mt-4 border-t border-border pt-3", data: { skim_list: true }) do
        if selectable
          label(class: "mb-1.5 flex w-fit cursor-pointer items-center gap-2 text-xs font-medium text-muted-foreground") do
            input(
              type: "checkbox",
              class: "h-3.5 w-3.5 cursor-pointer rounded border-gray-300 text-accent-600 focus:ring-accent-500",
              data: { skim_select_all: true, action: "change->skim-mode#toggleSelectAll" },
              aria_label: t(".select_all_aria")
            )
            span(data: { skim_selection_hint: true }) { t(".select_all") }
          end
        end

        div(class: "divide-y divide-border") do
          @emails.each { |em| email_row(em, selectable) }
        end
      end
    end

    # One expandable email row: header (sender · time · subject · preview) that
    # toggles an inline body panel below it.
    def email_row(em, selectable)
      div(class: "py-0.5", data: { skim_row: true, skim_id: em[:id] }) do
        div(class: "flex items-start gap-2") do
          if selectable
            input(
              type: "checkbox",
              value: em[:id],
              class: "mt-2.5 h-3.5 w-3.5 flex-shrink-0 cursor-pointer rounded border-gray-300 text-accent-600 focus:ring-accent-500",
              data: { skim_checkbox: true, action: "change->skim-mode#onSelectionChange" },
              aria_label: t(".select_email_aria", subject: em[:subject])
            )
          end
          button(
            type: "button",
            class: "block min-w-0 flex-1 -mx-1 cursor-pointer rounded-md px-1 py-1.5 text-left hover:bg-muted transition-colors",
            aria_expanded: "false",
            data: { action: "click->skim-mode#toggleRow" }
          ) do
            div(class: "flex items-center justify-between gap-2") do
              span(class: "min-w-0 flex-1 truncate text-xs font-medium text-muted-foreground") { em[:sender].to_s }
              div(class: "flex flex-shrink-0 items-center gap-1.5") do
                span(class: "text-[11px] text-muted-foreground/80 tabular-nums") { helpers.thread_date_label(em[:received_at]) } if em[:received_at]
                svg(class: "h-3.5 w-3.5 text-muted-foreground transition-transform", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true", data: { skim_chevron: true }) do
                  raw(safe('<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>'))
                end
              end
            end
            span(class: "block truncate text-sm text-foreground") { em[:subject].to_s }
            span(class: "block text-xs text-muted-foreground line-clamp-2", data: { skim_collapsed: true }) { em[:snippet] } if em[:snippet].to_s != ""
          end
        end
        inline_body(em)
      end
    end

    # Hidden until the row is expanded; then the whole body loads inline.
    def inline_body(em)
      div(class: "hidden pl-1", data: { skim_body: true }) do
        raw(safe(%(<turbo-frame id="skim_body_#{em[:id]}" data-skim-body-src="/skim/email/#{em[:id]}/content" class="block py-1 text-sm leading-relaxed text-foreground [&_a]:text-accent-600 [&_a]:underline [&_img]:max-w-full"><p class="text-xs text-muted-foreground">Loading…</p></turbo-frame>)))
        div(class: "mb-1 mt-1.5 flex items-center gap-3 text-xs font-medium") do
          a(href: "/skim/email/#{em[:id]}", class: "inline-flex items-center gap-1 text-accent-600 hover:underline", data: { turbo_frame: "skim_email_card", action: "click->skim-mode#openEmail" }) do
            plain t(".reply")
            span(aria_hidden: "true") { "↗" }
          end
          a(href: "/email_messages/#{em[:id]}", class: "inline-flex items-center gap-1 text-muted-foreground hover:underline", data: { turbo_frame: "_top" }) do
            plain t(".open_in_full")
            span(aria_hidden: "true") { "↗" }
          end
        end
      end
    end

    # Keep is the prominent, effortless default (advance + mark addressed so it
    # won't re-surface); Archive and the priority pin are the deliberate actions.
    # Pending cards lead with the allow/deny decision; starred cards offer Unstar;
    # every other card also exposes the sender-level Star / Block actions.
    def actions
      div(class: class_names("flex flex-wrap items-center gap-2", @fill ? "mt-auto pt-5" : "mt-5")) do
        case @theme
        when :follow_ups then follow_up_actions
        when :pending then pending_actions
        when :starred then starred_actions
        else default_actions
        end
      end
    end

    def pending_actions
      action_button(:allow, t(".allow"), "A", style: :primary)
      action_button(:deny, t(".deny"), "D", style: :danger)
      action_button(:keep, t(".decide_later"), "→", style: :secondary)
    end

    # The AI's "what you're waiting on" line, shown only on a follow-up card when a
    # reason was extracted. Sits under the summary as the spine of the nudge.
    def follow_up_note
      return unless @theme == :follow_ups && @follow_up_reason.present?

      div(class: "mt-2.5 flex items-start gap-1.5 text-xs text-muted-foreground") do
        svg(class: "mt-0.5 h-3.5 w-3.5 flex-shrink-0 text-amber-500", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[:clock])) }
        span { @follow_up_reason }
      end
    end

    # Follow-up card: AI-draft the nudge (opens the thread, where Scout leads with
    # "Draft follow-up" into the preview→approve→send composer), or dismiss the
    # follow-up so it stops surfacing in Skim and the feed.
    def follow_up_actions
      follow_up_draft_link
      action_button(:dismiss_follow_up, t(".dismiss_follow_up"), "D", style: :secondary, icon: :dismiss)
      action_button(:keep, t(".keep"), "→", style: :secondary)
    end

    # Primary action is a LINK (open the full thread), not a skim mutation — the AI
    # draft + send happens in the thread composer. turbo_frame _top leaves the overlay.
    def follow_up_draft_link
      id = @emails.first && @emails.first[:id]
      return unless id

      a(
        href: "/email_messages/#{id}?compose=follow_up",
        class: class_names("inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors", "bg-primary text-primary-foreground hover:bg-primary/90"),
        data: { turbo_frame: "_top" }
      ) do
        svg(class: "h-4 w-4 flex-shrink-0", fill: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[:sparkle])) }
        plain t(".draft_follow_up")
      end
    end

    def starred_actions
      action_button(:keep, t(".keep"), "→", style: :primary)
      action_button(:archive, archive_label, "E", style: :secondary)
      action_button(:unstar, t(".unstar"), "S", style: :secondary, icon: :star_filled)
    end

    # Normal cluster. When Scout has a learned pick, that verb leads as the primary
    # (sparkle-marked) button and Keep steps down to secondary; otherwise Keep leads
    # as the effortless default. Sender-level Star / Block stay available either way.
    def default_actions
      case scout_action
      when :archive
        action_button(:archive, archive_label, "E", style: :primary, suggested: true)
        action_button(:keep, t(".keep"), "→", style: :secondary)
        priority_button
        sender_actions
      when :promote
        promote_first_actions
      else
        keep_first_actions(mark_keep: scout_action == :keep)
      end
    end

    # Scout suggests pinning. If it's already pinned the pick is moot — fall back to
    # the default order (priority_button then renders Unpin).
    def promote_first_actions
      return keep_first_actions(mark_keep: false) if @pinned

      action_button(:promote, t(".make_priority"), "P", style: :primary, icon: :star, suggested: true)
      action_button(:keep, t(".keep"), "→", style: :secondary)
      action_button(:archive, archive_label, "E", style: :secondary)
      sender_actions
    end

    def keep_first_actions(mark_keep:)
      action_button(:keep, t(".keep"), "→", style: :primary, suggested: mark_keep)
      action_button(:archive, archive_label, "E", style: :secondary)
      priority_button
      sender_actions
    end

    def sender_actions
      action_button(:star, t(".star_sender"), "S", style: :secondary, icon: :star)
      action_button(:block, t(".block_sender"), "B", style: :secondary)
    end

    def archive_label
      @count == 1 ? t(".archive") : t(".archive_all")
    end

    # Scout's learned pick for this card as a verb symbol (:keep/:archive/:promote),
    # or nil. Only the recurring triage verbs are ever suggested.
    def scout_action
      return nil unless @scout_suggestion

      action = @scout_suggestion[:action].to_s.to_sym
      %i[keep archive promote].include?(action) ? action : nil
    end

    # A subtle Scout cue under the summary that explains the pre-selected action —
    # "✦ Scout · You usually archive these · 9 of your last 12". Spoken as Scout,
    # with the count so the suggestion is transparent rather than a black box.
    def scout_line
      action = scout_action
      return unless action

      div(class: "mt-2.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs") do
        span(class: "inline-flex items-center gap-1 rounded-full bg-accent-100 px-2 py-0.5 font-semibold text-accent-700 dark:bg-accent-500/15 dark:text-accent-300") do
          svg(class: "h-3 w-3", fill: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[:sparkle])) }
          plain t(".scout.lead")
        end
        span(class: "text-muted-foreground") { t(".scout.#{action}") }
        span(class: "text-muted-foreground/60 tabular-nums") { t(".scout.ratio", count: @scout_suggestion[:count], total: @scout_suggestion[:total]) }
      end
    end

    def priority_button
      if @pinned
        action_button(:unpromote, t(".unpin"), "P", style: :secondary, icon: :star_filled)
      else
        action_button(:promote, t(".make_priority"), "P", style: (@priority_suggested ? :accent : :secondary), icon: :star)
      end
    end

    def action_button(key, label, hint, style:, icon: nil, suggested: false)
      classes = case style
      when :primary then "bg-primary text-primary-foreground hover:bg-primary/90"
      when :accent  then "bg-accent-600 text-white hover:bg-accent-700"
      when :danger  then "border border-red-300 text-red-700 hover:bg-red-50 dark:border-red-500/40 dark:text-red-300 dark:hover:bg-red-500/10"
      else "border border-border text-foreground hover:bg-muted"
      end

      button(
        type: "button",
        class: class_names("inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors", classes),
        data: { skim_action: key }
      ) do
        if icon == :star || icon == :star_filled
          star_icon(filled: icon == :star_filled, klass: "h-4 w-4 flex-shrink-0")
        else
          svg(class: "h-4 w-4 flex-shrink-0", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[icon] || ICONS[key])) }
        end
        span(data: { skim_label: true }) { label }
        # A sparkle on Scout's pre-selected verb, tying the button to the Scout line.
        svg(class: "h-3 w-3 flex-shrink-0 opacity-90", fill: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[:sparkle])) } if suggested
        kbd(class: "ml-0.5 text-[10px] font-mono opacity-60") { hint }
      end
    end

    def star_icon(filled:, klass:)
      svg(
        class: klass,
        fill: filled ? "currentColor" : "none",
        stroke: "currentColor",
        viewBox: "0 0 24 24",
        aria_hidden: "true"
      ) { raw(safe(ICONS[filled ? :star_filled : :star])) }
    end
  end
end
