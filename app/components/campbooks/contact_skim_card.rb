# frozen_string_literal: true

module Campbooks
  # A single new-sender card in the contact Skim — the people analogue of
  # Campbooks::SkimCard's "pending sender" frame. It wears the same flat skim
  # chrome (solid bordered card, header pill + recency/volume meta, a bottom
  # action row) so the contact Skim reads as "every other skim function" rather
  # than a Tinder deck. The body carries a short filter legend (where Approve /
  # Block / Skip route this sender's mail) the way SkimCard's body carries the
  # emails being triaged — the contact Skim is a sender-filtering surface, so the
  # card spells the filtering out. The cross-fade, keyboard, and decision wiring
  # live in the `contact-skim` Stimulus controller; the card only carries the
  # Allow / Block / Skip buttons (data-skim-action) that bubble up to it. The
  # controller reads the decision URLs off the surrounding frame, so the card
  # stays endpoint-agnostic.
  # Copy is scoped to `contacts.skim.*` (shared with the deck view).
  class ContactSkimCard < Campbooks::Base
    ICONS = {
      allow: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>',
      block: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M18.364 5.636l-12.728 12.728M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>',
      skip:  '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 5l7 7-7 7M5 5l7 7-7 7"/>'
    }.freeze

    # @param contact [Contact]
    # @param fill [Boolean] tall "story frame" layout (the deck); false = compact
    # @param position/total [Integer, nil] story progress (compact / preview only —
    #   the deck shows the segmented stories bar in the shell instead)
    # @param show_progress [Boolean] render the built-in dots
    def initialize(contact:, fill: false, position: nil, total: nil, show_progress: false, **attrs)
      @contact = contact
      @fill = fill
      @position = position
      @total = total
      @show_progress = show_progress
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      div(
        class: class_names(
          "flex w-full flex-col rounded-xl border border-border bg-card shadow-sm",
          @fill ? "min-h-[26rem] max-h-[82vh] p-6 sm:min-h-[30rem]" : "max-w-sm p-5",
          custom
        ),
        **@attrs
      ) do
        progress if @show_progress && @position && @total
        header
        identity
        if @fill
          div(class: "mt-4 flex-1 overflow-y-auto overscroll-contain") do
            summary
            filter_guide
          end
        else
          div(class: "mt-3") { summary }
        end
        actions
      end
    end

    private

    # Mirrors SkimCard#header: a "New sender" bucket pill (+ relationship cue) on
    # the left; how recently and how often they've written on the right.
    def header
      div(class: "flex items-center justify-between gap-3") do
        div(class: "flex min-w-0 items-center gap-2") do
          span(class: "inline-flex flex-shrink-0 items-center rounded-full bg-muted px-2 py-0.5 text-[11px] font-semibold text-muted-foreground") do
            t("contacts.skim.new_sender")
          end
          raw(safe(helpers.relationship_badge(@contact.relationship_type))) if @contact.relationship_type.present?
        end
        div(class: "flex flex-shrink-0 items-center gap-2") do
          if @contact.last_email_at
            span(class: "text-xs text-muted-foreground tabular-nums") { helpers.thread_date_label(@contact.last_email_at) }
          end
          span(class: "text-xs text-muted-foreground tabular-nums") { t("contacts.skim.emails", count: @contact.email_count.to_i) }
        end
      end
    end

    # The card's "title" is a person: avatar + name + address (SkimCard leads with
    # a subject; here the human is the subject).
    def identity
      name = @contact.display_name.presence
      titled = name && name != @contact.email
      div(class: "mt-4 flex items-center gap-3") do
        avatar
        div(class: "min-w-0") do
          if titled
            h3(class: "truncate text-lg font-bold leading-tight text-foreground sm:text-xl") { name }
            p(class: "truncate text-sm text-muted-foreground") { @contact.email }
          else
            h3(class: "break-all text-base font-bold leading-snug text-foreground") { @contact.email }
          end
        end
      end
    end

    def avatar
      div(class: "flex h-12 w-12 flex-shrink-0 items-center justify-center rounded-full bg-accent-100 text-lg font-semibold text-accent-700 dark:bg-accent-500/15 dark:text-accent-300") do
        plain((@contact.display_name.presence || @contact.email).to_s.first.to_s.upcase.presence || "?")
      end
    end

    # Scout's read on the sender — the card's "so what". Falls back to a quiet line
    # so an unprofiled sender still reads as content, never a blank panel.
    def summary
      if @contact.context_summary.present?
        p(class: "text-sm leading-relaxed text-muted-foreground") { @contact.context_summary }
      else
        p(class: "text-sm text-muted-foreground/70") { t("contacts.skim.no_summary") }
      end
    end

    # A compact "what your choice does" legend. The contact Skim is a filtering
    # surface: each decision routes this sender's future mail, so the card spells
    # out where Approve / Block / Skip send it. This is the card's body content
    # (mirroring the emails SkimCard lists), and it makes the deck read as a
    # filtering tool rather than a blank profile card. Fill mode only.
    def filter_guide
      div(class: "mt-5 space-y-2.5 border-t border-border pt-4") do
        filter_guide_row(:allow, t("contacts.skim.approve"), t("contacts.skim.guide_approve"))
        filter_guide_row(:block, t("contacts.skim.block"),   t("contacts.skim.guide_block"))
        filter_guide_row(:skip,  t("contacts.skim.skip"),    t("contacts.skim.guide_skip"))
      end
    end

    def filter_guide_row(key, label, description)
      div(class: "flex items-start gap-2.5") do
        span(class: "mt-0.5 flex h-5 w-5 flex-shrink-0 items-center justify-center rounded-full bg-muted text-muted-foreground") do
          svg(class: "h-3 w-3", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[key])) }
        end
        p(class: "text-sm leading-snug text-muted-foreground") do
          strong(class: "font-semibold text-foreground") { label }
          plain " — #{description}"
        end
      end
    end

    # Same row, same vocabulary as SkimCard's pending frame: Allow is the near-black
    # primary, Block is the destructive cue, Skip defers. The controller reads the
    # decision URLs off the frame; these buttons only name the action.
    def actions
      div(class: class_names("flex flex-wrap items-center gap-2", @fill ? "mt-auto pt-5" : "mt-5")) do
        action_button(:allow, t("contacts.skim.approve"), "A", style: :primary)
        action_button(:block, t("contacts.skim.block"), "B", style: :danger)
        action_button(:skip, t("contacts.skim.skip"), "→", style: :secondary)
      end
    end

    def action_button(key, label, hint, style:)
      classes = case style
      when :primary then "bg-primary text-primary-foreground hover:bg-primary/90"
      when :danger  then "border border-red-300 text-red-700 hover:bg-red-50 dark:border-red-500/40 dark:text-red-300 dark:hover:bg-red-500/10"
      else "border border-border text-foreground hover:bg-muted"
      end

      button(
        type: "button",
        class: class_names("inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors", classes),
        data: { skim_action: key }
      ) do
        svg(class: "h-4 w-4 flex-shrink-0", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[key])) }
        span(data: { skim_label: true }) { label }
        kbd(class: "ml-0.5 text-[10px] font-mono opacity-60") { hint }
      end
    end

    # Dots fallback for the compact / preview card; the deck uses the shell's
    # segmented stories bar (show_progress: false) instead.
    def progress
      div(class: "mb-4 flex items-center gap-1", aria_label: t("contacts.skim.progress", done: @position, total: @total)) do
        (1..@total).each do |i|
          div(class: class_names("h-1 rounded-full transition-all", i == @position ? "w-6 bg-foreground" : "w-1.5 bg-muted"))
        end
      end
    end
  end
end
