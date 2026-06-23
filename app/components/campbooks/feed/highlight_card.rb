# frozen_string_literal: true

module Campbooks
  module Feed
    # A past highlight in the home feed's Rewind (Feed::Rewind). It IS a feed
    # email card — same flat article, avatar, 17px subject, body preview, Open as
    # EmailActionCard — but it leads with a REASON kicker naming why it resurfaced
    # (starred sender, important, attachment, busy thread). That kicker is the
    # point: a rewind card is never hollow, it always answers "why am I seeing
    # this?" the way DESIGN.md's Meaning Rule demands.
    #
    # Reason styling follows the system: Ember (the priority dot) for the "wants
    # you / matters" signals (important, high priority), the gold star accent for
    # starred senders, neutral chips for the structural ones (attachment, thread).
    # Takes a bare email + its reason symbol (computed by Feed::Rewind), not a
    # feed_item — the rewind isn't materialized.
    class HighlightCard < Campbooks::Feed::Base
      STAR = '<path d="M11.48 3.5a.56.56 0 011.04 0l2.12 4.92 5.34.46c.49.04.69.66.31.98l-4.05 3.5 1.21 5.22c.11.48-.41.86-.83.6L12 17.27l-4.63 2.91c-.42.26-.94-.12-.83-.6l1.21-5.22-4.05-3.5c-.38-.32-.18-.94.31-.98l5.34-.46 2.12-4.92z"/>'

      def initialize(email:, reason:)
        @subject = email
        @reason = reason.to_sym
      end

      def view_template
        # Swipe-left to archive on touch — mirrors the curated feed's Feed::Card.
        # The wrapper carries the id EmailToolsController removes on a rewind
        # archive (and re-inserts on undo); it isn't a materialized feed item.
        render Campbooks::Swipeable.new(
          id: "rewind_highlight_#{subject.id}",
          class: "py-9",
          data: { feed_focus_unit: true },
          surface: "var(--color-background)",
          left: [ archive_stage ],
          right: []
        ) do
          article do
            reason_kicker
            header_row
            h3(class: "mt-2.5 text-[17px] font-semibold leading-snug tracking-tight text-foreground") do
              a(href: helpers.email_message_path(subject),
                class: "rounded-sm outline-none transition-colors hover:text-foreground/70 focus-visible:ring-2 focus-visible:ring-ring") do
                clean_subject(subject)
              end
            end
            email_body_preview(subject, margin: "mt-2.5")
            meta_row
            action_bar
          end
        end
      end

      private

      attr_reader :reason

      # The "why kept" line — the first thing read on the card.
      def reason_kicker
        case reason
        when :starred
          kicker(:accent) { star_glyph; plain t(".reason.starred") }
        when :important
          kicker(:ember) { ember_dot; plain t(".reason.important") }
        when :high_priority
          kicker(:ember) { ember_dot; plain t(".reason.high_priority") }
        when :attachment
          kicker(:neutral) { raw safe(clip_icon); plain t(".reason.attachment") }
        when :busy_thread
          kicker(:neutral) { raw safe(thread_icon); plain t(".reason.busy_thread") }
        end
      end

      # Tone sets the kicker's color: accent = gold (starred), ember = the warm
      # signature (priority / matters), neutral = quiet gray (structural).
      def kicker(tone, &block)
        classes = {
          accent: "text-accent-700 dark:text-accent-300",
          ember: "text-foreground",
          neutral: "text-muted-foreground"
        }.fetch(tone)
        div(class: "mb-2 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-[0.08em] #{classes}", &block)
      end

      def header_row
        participants = thread_participants(subject)
        multi = participants.size > 1

        div(class: "flex items-center gap-3") do
          if multi
            render Campbooks::ContactAvatarGroup.new(participants: participants, size: :xl, variant: :neutral, max: 3)
          else
            render Campbooks::ContactAvatar.new(
              email: subject.from_address, contact_id: subject.contact_id, size: :xl, variant: :neutral
            )
          end
          div(class: "min-w-0 flex-1") do
            div(class: "truncate text-[13px] font-semibold text-foreground") do
              multi ? participants_label(participants) : sender_name(subject)
            end
            div(class: "text-[11.5px] text-muted-foreground") { relative_time(subject.received_at || subject.created_at) }
          end
        end
      end

      # Structural chips that aren't already the reason (avoid saying it twice).
      def meta_row
        show_thread = thread_count > 1 && reason != :busy_thread
        show_attach = subject.has_attachment? && reason != :attachment
        return unless show_thread || show_attach

        div(class: "mt-3 flex flex-wrap gap-1.5") do
          thread_chip if show_thread
          attachment_chip if show_attach
        end
      end

      def thread_count
        @thread_count ||= subject.email_thread&.email_messages&.size.to_i
      end

      def action_bar
        div(class: "mt-3.5 flex flex-wrap items-center justify-end gap-2") do
          archive_button
          link_button(href: helpers.email_message_path(subject),
                      label: t("components.feed.email_action_card.open"), variant: :primary)
        end
      end

      # Archive posts straight to the email (Rewind cards aren't materialized feed
      # items, so they can't use Feed::ItemsController#act). surface:"rewind" tells
      # EmailToolsController to remove THIS card + offer an undo that re-inserts it.
      def archive_button
        action_form(helpers.tool_email_message_path(subject),
                    fields: { tool: "archive", surface: "rewind", reason: reason }) do
          render Campbooks::Button.new(
            variant: :ghost, size: :sm, type: "submit",
            title: t("components.feed.email_action_card.archive_hint"),
            data: feed_action_attrs(dismiss: true).merge(turbo_submits_with: t("components.feed.shared.working"))
          ) do
            plain t("components.feed.email_action_card.archive")
            key_chip(dismiss: true)
          end
        end
      end

      # The swipe-left stage — same endpoint/params as the button (one source of
      # truth for the action), so a swipe and a tap do the identical thing.
      def archive_stage
        { key: "archive", label: t("components.feed.email_action_card.archive"), icon: :archive,
          color: "neutral", endpoint: helpers.tool_email_message_path(subject),
          params: { "tool" => "archive", "surface" => "rewind", "reason" => reason.to_s } }
      end

      def star_glyph
        svg(class: "h-3.5 w-3.5", fill: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw safe(STAR) }
      end

      def ember_dot
        span(class: "h-1.5 w-1.5 flex-shrink-0 rounded-full", style: "background-color: var(--ember-solid)")
      end

      def thread_chip
        span(class: "inline-flex items-center gap-1.5 rounded-lg border border-border bg-muted px-2.5 py-1 text-[11.5px] font-medium text-foreground/80") do
          raw safe(thread_icon)
          span { t("components.feed.email_action_card.messages", count: thread_count) }
        end
      end

      def attachment_chip
        span(class: "inline-flex items-center gap-1.5 rounded-lg border border-border bg-muted px-2.5 py-1 text-[11.5px] font-medium text-foreground/80") do
          raw safe(clip_icon)
          span { t("components.feed.email_action_card.attachment") }
        end
      end

      def thread_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3 w-3 text-muted-foreground"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>)
      end

      def clip_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3 w-3 text-muted-foreground"><path d="M21 9 12 18a4 4 0 0 1-6-6l8-8a3 3 0 0 1 4 4l-8.5 8.5"/></svg>)
      end
    end
  end
end
