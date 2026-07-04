# frozen_string_literal: true

module Campbooks
  module Feed
    # The most prominent feed card: an email from a starred sender. Where the rest
    # of the feed is flat (content on the canvas), this one wears its own surface —
    # accent border + star badge — because a starred sender outweighs ordinary mail.
    # Each is its own card (never grouped with other senders).
    class StarredEmailCard < Campbooks::Feed::Base
      STAR = '<path d="M11.48 3.5a.56.56 0 011.04 0l2.12 4.92 5.34.46c.49.04.69.66.31.98l-4.05 3.5 1.21 5.22c.11.48-.41.86-.83.6L12 17.27l-4.63 2.91c-.42.26-.94-.12-.83-.6l1.21-5.22-4.05-3.5c-.38-.32-.18-.94.31-.98l5.34-.46 2.12-4.92z"/>'

      def view_template
        article(class: "rounded-2xl border border-accent-300/70 bg-card p-5 shadow-sm transition-shadow hover:shadow-md") do
          header_row
          h3(class: "mt-3.5 text-[17px] font-semibold leading-snug tracking-tight text-foreground") do
            a(href: helpers.email_message_path(subject), class: "rounded-sm outline-none transition-colors hover:text-foreground/70 focus-visible:ring-2 focus-visible:ring-ring") { clean_subject(subject) }
          end
          body
          sender_tag_chips
          action_bar
        end
      end

      private

      def header_row
        participants = thread_participants(subject)
        multi = participants.size > 1

        div(class: "flex items-center gap-3") do
          if multi
            render Campbooks::ContactAvatarGroup.new(participants: participants, size: :xl, variant: :accent, max: 3)
          else
            render Campbooks::ContactAvatar.new(
              email: subject.from_address, contact_id: subject.contact_id, size: :xl, variant: :accent
            )
          end
          div(class: "min-w-0 flex-1") do
            div(class: "flex items-center gap-1.5") do
              star_badge
              span(class: "truncate text-[13px] font-semibold text-foreground") do
                multi ? participants_label(participants) : sender_name(subject)
              end
            end
            div(class: "text-[11.5px] text-muted-foreground") { relative_time(subject.received_at) }
          end
          priority_dot if item.attention?
        end
      end

      # A small gold "Starred" pill so the promotion reads at a glance.
      def star_badge
        span(class: "inline-flex flex-shrink-0 items-center gap-1 rounded-full bg-accent-100 px-2 py-0.5 text-[10.5px] font-semibold text-accent-700 dark:bg-accent-500/15 dark:text-accent-300") do
          svg(class: "h-3 w-3", fill: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw safe(STAR) }
          plain t(".starred")
        end
      end

      # Scout's read leads when present, as a single Ember line, with the full
      # message one collapsed tap below; otherwise the email excerpt is the body.
      def body
        if (msg = scout_message).present?
          render Campbooks::ScoutNote.new(message: msg, compact: true, class: "mt-3")
          render Campbooks::Feed::ExpandablePreview.new(item: item, class: "mt-2")
        else
          email_body_preview(subject, margin: "mt-3")
        end
      end

      def scout_message
        @scout_message ||= subject.ai_action_prompt.presence || subject.ai_summary.presence
      end

      # The sender's characteristic tags (AI-assigned) — context for why this
      # sender matters and how their mail tends to file.
      def sender_tag_chips
        tags = Array(subject.contact&.sender_tags).first(3)
        return if tags.empty?

        div(class: "mt-3.5 flex flex-wrap gap-1.5") do
          tags.each do |tag|
            span(class: "inline-flex items-center gap-1.5 rounded-lg bg-muted px-2.5 py-1 text-[11.5px] font-medium text-foreground/80") do
              span(class: "h-2 w-2 flex-shrink-0 rounded-full", style: "background-color: #{css_color(tag.color)}") if tag.color.present?
              plain tag.name
            end
          end
        end
      end

      # Open is the primary (read the whole thing); Archive and Unstar are escapes.
      def action_bar
        div(class: "mt-4 flex flex-wrap items-center justify-end gap-2") do
          act_button(tool: "unstar_sender", label: t(".unstar"), variant: :ghost, key: "s")
          act_button(tool: "archive", label: t(".archive"), variant: :ghost, hint: t(".archive_hint"), key: "e", dismiss: true)
          link_button(href: helpers.email_message_path(subject), label: t(".open"), variant: :primary, key: "o")
        end
      end

      # Only allow a safe CSS color token through inline style (hex or simple name).
      def css_color(value)
        value.to_s =~ /\A#?[A-Za-z0-9]+\z/ ? value : "transparent"
      end
    end
  end
end
