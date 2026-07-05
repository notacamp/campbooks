# frozen_string_literal: true

module Campbooks
  module Feed
    # The hero card: a Scout-flagged email with its read and one-tap actions.
    # The substantial, dominant unit of the feed — sender → subject → Scout's
    # one-line read → meta → right-aligned action bar (primary solid-ink far right).
    # Flat (no border/shadow/surface): the feed reads as content on the canvas,
    # separated by whitespace, with vertical rhythm owned by the home timeline.
    class EmailActionCard < Campbooks::Feed::Base
      def view_template
        article do
          header_row
          h3(class: "mt-3 text-[17px] font-semibold leading-snug tracking-tight text-foreground") do
            a(href: helpers.email_message_path(subject), class: "rounded-sm outline-none transition-colors hover:text-foreground/70 focus-visible:ring-2 focus-visible:ring-ring") { clean_subject(subject) }
          end
          # Scout leads with a short read (AI does the boring part, visibly), capped
          # at three lines with a "Read more" — no raw-email preview beneath it, that
          # duplicated what Scout already said. A collapsed "Show email" keeps the full
          # message one tap away, so the decision never requires leaving the feed. If
          # Scout had nothing to say, the email excerpt IS the body and needs no disclosure.
          if scout_message.present?
            render Campbooks::ScoutNote.new(message: scout_message, compact: true, lines: 3, class: "mt-2.5")
            render Campbooks::Feed::ExpandablePreview.new(item: item, class: "mt-2")
          else
            email_body_preview(subject, margin: "mt-2.5")
          end
          meta_row
          action_bar
        end
      end

      private

      def header_row
        participants = thread_participants(subject, known_count: thread_count)
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
            div(class: "text-[11.5px] text-muted-foreground") { relative_time(subject.received_at) }
          end
          priority_dot if item.attention?
        end
      end

      def meta_row
        return unless subject.category.present? || subject.has_attachment? || thread_count > 1

        div(class: "mt-3 flex flex-wrap gap-1.5") do
          category_chip if subject.category.present?
          thread_chip if thread_count > 1
          attachment_chip if subject.has_attachment?
        end
      end

      def thread_count = item.data["thread_count"].to_i

      def thread_chip
        span(class: "inline-flex items-center gap-1.5 rounded-lg border border-border bg-muted px-2.5 py-1 text-[11.5px] font-medium text-foreground/80") do
          raw safe(thread_icon)
          span { t(".messages", count: thread_count) }
        end
      end

      def thread_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3 w-3 text-muted-foreground"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>)
      end

      def category_chip
        span(class: "inline-flex items-center rounded-lg bg-muted px-2.5 py-1 text-[11.5px] font-semibold text-foreground/75") do
          plain "#"
          plain subject.category.to_s.tr("_", " ")
        end
      end

      def attachment_chip
        span(class: "inline-flex items-center gap-1.5 rounded-lg border border-border bg-muted px-2.5 py-1 text-[11.5px] font-medium text-foreground/80") do
          raw safe(clip_icon)
          span { t(".attachment") }
        end
      end

      def scout_message
        @scout_message ||= subject.ai_action_prompt.presence || subject.ai_summary.presence
      end

      # Two actions only (DESIGN.md: primary = Scout's suggested action + one ghost
      # escape). When Scout suggests a tag, filing is the primary; else Open is. The
      # subject links to the email, so opening stays one tap even when primary = File.
      def action_bar
        div(class: "mt-3.5 flex flex-wrap items-center justify-end gap-2") do
          act_button(tool: "archive", label: t(".archive"), variant: :ghost, hint: t(".archive_hint"), key: "e", dismiss: true)
          if (tag = suggested_tag_name)
            act_button(tool: "add_tag", args: { tag_name: tag }, variant: :primary, label: t(".file_as", tag: tag), key: "c", primary: true)
          else
            link_button(href: helpers.email_message_path(subject), label: t(".open"), variant: :primary, key: "o")
          end
        end
      end

      def suggested_tag_name
        action = Array(subject.ai_suggested_actions).find { |a| a["tool"] == "add_tag" }
        action&.dig("args", "tag_name").to_s.presence
      end

      def clip_icon
        %(<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3 w-3 text-muted-foreground"><path d="M21 9 12 18a4 4 0 0 1-6-6l8-8a3 3 0 0 1 4 4l-8.5 8.5"/></svg>)
      end
    end
  end
end
