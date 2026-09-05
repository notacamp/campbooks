# frozen_string_literal: true

module Campbooks
  module TimePage
    # Scout's day note at the top of the agenda: the warm scout-glass card from the
    # mock (Ember avatar · "Scout" · AI badge · "today") over one to three honest
    # sentences assembled from Time::DayNote's facts — counts of today's meetings and
    # deadlines, the focus block Scout is holding, and (only when the Money PR is
    # present) the most overdue obligation. Never states more than the rows prove.
    class DayNote < Campbooks::Base
      # @param note [::Time::DayNote::Result]
      # @param zone [ActiveSupport::TimeZone]
      def initialize(note:, zone:)
        @note = note
        @zone = zone
      end

      def view_template
        div(class: "scout-glass rounded-2xl px-4 py-3.5") do
          header
          p(class: "mt-2 text-[13.5px] leading-relaxed text-foreground/85") { plain sentences }
        end
      end

      private

      def header
        div(class: "flex items-center gap-2 text-[13px] font-bold text-foreground") do
          render Campbooks::ScoutAvatar.new(size: :xs)
          span { "Scout" }
          span(class: "rounded bg-ember-gradient px-1.5 py-0.5 text-[9.5px] font-bold tracking-wide text-white") { "AI" }
          span(class: "ml-auto text-[11px] font-normal text-muted-foreground") { t(".today") }
        end
      end

      def sentences
        parts = [ counts_sentence, focus_sentence, late_sentence, undated_sentence ].compact
        parts.empty? ? t(".clear") : parts.join(" ")
      end

      # "One ask has no date yet." / "N asks have no date yet." — honest, no
      # "say the word" copy (that lives in Scout's chat, not the day note).
      def undated_sentence
        count = @note.undated_count
        return unless count.positive?

        t(".undated", count: count)
      end

      # "Two meetings and one deadline today." — degrades gracefully when only one
      # side is present, and is dropped entirely on an empty day (the focus / late
      # sentences can still carry the note).
      def counts_sentence
        m = @note.meetings_count
        d = @note.deadlines_count
        meetings = t(".meetings", count: m) if m.positive?
        deadlines = t(".deadlines", count: d) if d.positive?
        if meetings && deadlines
          t(".summary_both", meetings: meetings, deadlines: deadlines)
        elsif meetings
          t(".summary_one", phrase: meetings)
        elsif deadlines
          t(".summary_one", phrase: deadlines)
        end
      end

      def focus_sentence
        focus = @note.focus
        return unless focus

        t(".focus",
          duration: t(".minutes", count: focus.duration_minutes),
          when: focus_when(focus.at),
          subject: focus.subject)
      end

      def focus_when(at)
        local = at.in_time_zone(@zone)
        "#{relative_day(local.to_date)} #{t('.at')} #{local.strftime('%H:%M')}"
      end

      def relative_day(date)
        today = ::Time.current.in_time_zone(@zone).to_date
        case date
        when today then t(".today_word")
        when today + 1 then t(".tomorrow_word")
        else l(date, format: "%A")
        end
      end

      def late_sentence
        late = @note.late_obligation
        return unless late

        t(".late", name: late.name, count: late.days_overdue)
      end
    end
  end
end
