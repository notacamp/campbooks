# frozen_string_literal: true

module Campbooks
  module Now
    # One row of Scout's log under the deck: a mono timestamp, a sentence (the
    # event's label · its subject), and the row's actions — Undo where a reverse
    # genuinely exists (archive → unarchive, tag → remove_tag), Open where the
    # subject has a page. Reuses Campbooks::Activity::EventPresentation so the log
    # tells the same story the Activity timeline does. `undone: true` renders the
    # muted post-undo state the undo turbo-stream swaps in.
    class LogRow < Campbooks::Base
      include Campbooks::Activity::EventPresentation

      def initialize(event:, undone: false)
        @event = event
        @undone = undone
      end

      def view_template
        div(
          id: helpers.dom_id(@event),
          class: "grid grid-cols-[44px_minmax(0,1fr)_auto] items-center gap-3 border-b border-border py-2.5 last:border-b-0 sm:grid-cols-[52px_minmax(0,1fr)_auto]"
        ) do
          span(class: "font-mono text-[11.5px] tabular-nums text-muted-foreground") { timestamp }
          @undone ? undone_sentence : sentence
          actions
        end
      end

      private

      def timestamp
        @event.occurred_at ? @event.occurred_at.strftime("%H:%M") : ""
      end

      def sentence
        span(class: "min-w-0 truncate text-[13.5px] text-foreground") do
          plain(label)
          if (detail = payload_summary).present?
            plain(" · ")
            span(class: "font-semibold") { detail }
          end
        end
      end

      def undone_sentence
        span(class: "min-w-0 truncate text-[13.5px] text-muted-foreground") { t(".undone") }
      end

      def actions
        span(class: "flex justify-end gap-3 text-xs text-muted-foreground") do
          next if @undone

          undo_form if reversible?
          open_link if subject_path
        end
      end

      LINK_CLASSES = "cursor-pointer border-b border-border pb-px text-muted-foreground no-underline transition-colors hover:text-foreground"

      def undo_form
        form(action: helpers.now_log_undo_path(@event), method: "post", class: "contents", data: { turbo_stream: true }) do
          input(type: "hidden", name: "authenticity_token", value: helpers.form_authenticity_token)
          button(type: "submit", class: LINK_CLASSES) { t(".undo") }
        end
      end

      def open_link
        a(href: subject_path, class: LINK_CLASSES) { t(".open") }
      end

      # Only where a reverse exists today AND the subject is an EmailMessage the
      # user can act on (the log is already accessible_to-scoped, so a listed
      # EmailMessage subject is reachable). Checked off subject_type/id — no load.
      def reversible?
        case @event.name
        when "email.archived" then subject_email?
        when "email.tagged"   then subject_email? && @event.payload["tag"].present?
        else false
        end
      end

      def subject_email?
        @event.subject_type == "EmailMessage" && @event.subject_id.present?
      end
    end
  end
end
