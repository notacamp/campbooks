# frozen_string_literal: true

module Campbooks
  module Activity
    # One row in the workspace activity timeline: an icon tile, the event's human
    # label (linked to the subject's page for stable record types), a secondary
    # line (actor · key payload detail), and a relative timestamp.
    #
    # The label resolves via i18n (events.names.<key>) with the registry's English
    # label as the default, and falls back to a humanized name for unregistered
    # event types — so a custom emit_event still reads sensibly.
    class EventRow < Campbooks::Base
      # label / subject_path / payload_summary / icon_key / ICONS — shared with
      # Campbooks::Now::LogRow so the Activity timeline and Scout's log agree.
      include Campbooks::Activity::EventPresentation

      def initialize(event:)
        @event = event
      end

      def view_template
        div(class: "flex items-start gap-3 px-4 py-3") do
          icon_tile
          div(class: "min-w-0 flex-1") do
            div(class: "flex items-baseline justify-between gap-3") do
              title_node
              time(
                class: "flex-shrink-0 whitespace-nowrap text-xs text-muted-foreground",
                datetime: @event.occurred_at&.iso8601
              ) { relative_time }
            end
            detail = detail_line
            p(class: "mt-0.5 truncate text-xs text-muted-foreground") { detail } if detail.present?
          end
        end
      end

      private

      def title_node
        text = label
        if (path = subject_path)
          a(href: path, class: "truncate text-sm font-medium text-foreground hover:underline") { text }
        else
          p(class: "truncate text-sm font-medium text-foreground") { text }
        end
      end

      def icon_tile
        span(class: "flex size-9 flex-shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground") do
          icon_svg
        end
      end

      def detail_line
        [ actor_label, payload_summary ].compact.reject(&:blank?).join(" · ")
      end

      def actor_label
        actor = @event.actor
        return t("events.system_actor") if actor.nil?
        return actor.name if actor.respond_to?(:name) && actor.name.present?
        return actor.email_address if actor.respond_to?(:email_address) && actor.email_address.present?

        actor.class.model_name.human
      end

      def relative_time
        return "" unless @event.occurred_at

        t("events.time_ago", time: helpers.time_ago_in_words(@event.occurred_at))
      end
    end
  end
end
