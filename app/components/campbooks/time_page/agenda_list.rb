# frozen_string_literal: true

module Campbooks
  module TimePage
    # The bold Time agenda: day sections (TODAY / TOMORROW / FRIDAY / a full date)
    # each listing the merged Time::AgendaItem rows, then a final "No date yet"
    # section for undated asks, then the snoozed threads and scheduled emails that
    # already rode the classic agenda (nothing lost). Empty days are skipped —
    # except today, which shows "Nothing scheduled." Wraps the #time_agenda
    # container so a focus/ask action can replace it in place.
    class AgendaList < Campbooks::Base
      def initialize(items:, zone:, undated: [], move_slots: {}, hold_slots: {}, snoozed_threads: [], scheduled_emails: [], has_calendars: true)
        @items = Array(items)
        @undated = Array(undated)
        @move_slots = move_slots || {}
        @hold_slots = hold_slots || {}
        @snoozed_threads = Array(snoozed_threads)
        @scheduled_emails = Array(scheduled_emails)
        @zone = zone
        @has_calendars = has_calendars
      end

      def view_template
        section(id: "time_agenda", class: "mt-6 space-y-7") do
          days.each { |day| render_day(day) }
          render_undated
        end
      end

      private

      # The "No date yet" section: undated asks, with the three ways out on each row
      # (Set a date / Hold / Done). Same h3 grammar as a day section, plus a muted
      # count and a right-aligned hint.
      def render_undated
        return if @undated.empty?

        section do
          div(class: "mb-1 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-0.5") do
            h3(class: "text-xs font-semibold uppercase tracking-wide text-muted-foreground") do
              plain t(".no_date_yet")
              span(class: "ml-1.5 font-normal normal-case tracking-normal text-muted-foreground/70") { "· #{@undated.size}" }
            end
            span(class: "text-[11px] font-normal normal-case tracking-normal text-muted-foreground") { t(".no_date_hint") }
          end
          div { @undated.each { |item| render_item(item) } }
        end
      end

      def render_day(day)
        rows = items_by_day[day] || []
        snoozed = snoozed_by_day[day] || []
        scheduled = scheduled_by_day[day] || []
        return if day != today && rows.empty? && snoozed.empty? && scheduled.empty?

        section do
          h3(class: "mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground") { day_label(day) }
          if rows.empty? && snoozed.empty? && scheduled.empty?
            p(class: "py-3 text-sm text-muted-foreground") { t(".empty_today") }
          else
            div do
              rows.each { |item| render_item(item) }
              snoozed.each { |thread| render Campbooks::Calendar::SnoozedRow.new(thread: thread) }
              scheduled.each { |email| render Campbooks::Calendar::ScheduledEmailRow.new(scheduled_email: email) }
            end
          end
        end
      end

      def render_item(item)
        render Campbooks::TimePage::AgendaRow.new(
          item: item, zone: @zone,
          move_slots: item.focus? ? Array(@move_slots[item.record.id]) : [],
          hold_slot: item.task? ? @hold_slots[item.record.id] : nil
        )
      end

      # ── grouping ───────────────────────────────────────────────────────────────
      def items_by_day
        @items_by_day ||= @items.group_by(&:day)
      end

      def snoozed_by_day
        @snoozed_by_day ||= @snoozed_threads.group_by { |t| t.snoozed_until.in_time_zone(@zone).to_date }
      end

      def scheduled_by_day
        @scheduled_by_day ||= @scheduled_emails.group_by { |s| (s.next_occurrence_at || s.scheduled_at).in_time_zone(@zone).to_date }
      end

      # Every day that has something, plus today (always shown even when empty),
      # ascending.
      def days
        (items_by_day.keys + snoozed_by_day.keys + scheduled_by_day.keys + [ today ]).uniq.sort
      end

      def today
        @today ||= ::Time.current.in_time_zone(@zone).to_date
      end

      # Today / Tomorrow / a weekday name within the week / a full date beyond it.
      def day_label(date)
        case date
        when today then t(".today")
        when today + 1 then t(".tomorrow")
        else date < today + 7 ? l(date, format: "%A") : l(date, format: :full)
        end
      end
    end
  end
end
