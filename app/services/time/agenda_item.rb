# frozen_string_literal: true

# One row of the bold Time agenda, whatever it came from: a calendar event, a
# deadline Scout found in mail (a pending Reminder), a due-dated Task, or a
# proposed FocusBlock. The whole point of the Rethink's Time surface is that these
# are the same thing — a commitment with a time — so they render in one column.
#
# Built by Time::Agenda. Fields:
# - kind             :event | :deadline | :task | :focus
# - at               the instant it happens (start_at / due_at)
# - day              the Date it buckets under, in the user's zone (overdue
#                    deadlines bucket under today — pinned, never dropped)
# - all_day          renders "all day" instead of a clock time
# - overdue          a past-due deadline pinned into today
# - duration_minutes 0 for deadlines/tasks (no span)
# - title            the row's headline
# - source_label     provenance meta ("30 min · Google Calendar" builds the "min"
#                    from duration; this is the "Google Calendar" / "from Ana's
#                    email" / "Scout suggested" / "Task" part)
# - source_path      the primary back-link (email thread / document), or nil
# - color            hex for an event's calendar colour; nil for deadline (ember),
#                    focus (dashed) and task (ink outline) — the row draws those
# - record           the underlying AR record (EventRow-style edit links, etc.)
# - actions          available row actions, in order (see Time::Agenda)
#
# Namespaced under Ruby's core Time (reopened via the compact Time::AgendaItem
# form — never a bare `class Time`, which would clash class-vs-module).
class Time::AgendaItem < Data.define(
  :kind, :at, :day, :all_day, :overdue, :duration_minutes,
  :title, :source_label, :source_path, :color, :record, :actions
)
  def event? = kind == :event
  def deadline? = kind == :deadline
  def task? = kind == :task
  def focus? = kind == :focus

  # An undated ask (the "No date yet" section): no day, no instant. These are
  # served by Time::Agenda#undated, never through #items.
  def undated? = day.nil?

  # Sort key: within a day, all-day items come first, then by instant. Days
  # ascending. Overdue deadlines carry day == today, so they head today's list.
  # Tolerates a nil day (undated asks) by sorting it last, though those never go
  # through #items.
  def sort_key
    [ day.nil? ? 1 : 0, day || ::Date.new(0), all_day ? 0 : 1, at || ::Time.at(0) ]
  end

  def action?(name)
    actions.include?(name)
  end
end
