# frozen_string_literal: true

# Value object around an RFC 5545 RRULE string, shared by every recurring
# surface in the app (calendar events, tasks). It wraps ice_cube for occurrence
# math and RRULE (de)serialization so we round-trip the *same* rule string that
# Google/Zoho calendars speak.
#
# The UI never exposes a raw RRULE: it offers a curated set of PRESETS (a plain
# `<select>`, see Campbooks::RecurrencePicker) whose values are the anchor-free
# rule strings below. "Anchor-free" matters — a bare `FREQ=WEEKLY` repeats on the
# start time's own weekday and `FREQ=MONTHLY` on its day-of-month, in both
# Google's and ice_cube's semantics, so the one stored string means the right
# thing wherever the record's start time lands. A rule that doesn't match a
# preset (e.g. an arbitrary RRULE synced in from a provider) still parses and
# expands fine; it just reports #preset_key == :custom.
#
#   Recurrence.new("FREQ=WEEKLY").next_occurrence(dtstart: t, after: Time.current)
#   Recurrence.wrap(task.rrule).occurrences_between(dtstart: s, from:, to:)
class Recurrence
  # key => canonical RRULE. Order is the order the picker renders them.
  PRESETS = {
    daily: "FREQ=DAILY",
    weekdays: "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR",
    weekly: "FREQ=WEEKLY",
    biweekly: "FREQ=WEEKLY;INTERVAL=2",
    monthly: "FREQ=MONTHLY",
    yearly: "FREQ=YEARLY"
  }.freeze

  # Safety cap on any single expansion so an unbounded rule + wide window can
  # never spin forever or balloon memory.
  MAX_OCCURRENCES = 366

  attr_reader :rrule

  # Accepts a Recurrence (returned as-is), a raw rrule string, or nil.
  def self.wrap(value)
    value.is_a?(self) ? value : new(value)
  end

  # The rrule string for a preset key (symbol or string), or nil if unknown.
  def self.preset_rrule(key)
    PRESETS[key.to_s.to_sym]
  end

  # [[key, rrule], ...] for building the picker; labels are resolved in the view.
  def self.preset_options
    PRESETS.to_a
  end

  # Is this a value we can store? Blank (non-recurring) or a parseable rule.
  def self.valid?(value)
    wrap(value).valid?
  end

  def initialize(rrule)
    # Tolerate a leading "RRULE:" — providers hand us the rule with that prefix.
    @rrule = rrule.to_s.strip.delete_prefix("RRULE:").presence
  end

  def present?
    rrule.present?
  end
  alias_method :recurring?, :present?

  def blank?
    rrule.blank?
  end

  def ==(other)
    other.is_a?(self.class) && other.rrule == rrule
  end
  alias_method :eql?, :==

  def hash
    rrule.hash
  end

  # :daily/:weekly/... for a known preset, :custom for any other parseable rule,
  # or nil when there's no rule at all. Matches the raw stored string (the picker
  # only ever stores exact preset values), falling back to :custom otherwise.
  def preset_key
    return nil if blank?
    PRESETS.key(rrule) || :custom
  end

  # Non-blank and parseable. A blank rule is "valid" (it just means no recurrence).
  def valid?
    blank? || !rule.nil?
  end

  # The parsed ice_cube rule, or nil if blank/unparseable.
  def rule
    return nil if blank?
    @rule = IceCube::Rule.from_ical(rrule) unless defined?(@rule)
    @rule
  rescue StandardError => e
    Rails.logger.warn("[Recurrence] unparseable rrule #{rrule.inspect}: #{e.message}") if defined?(Rails)
    @rule = nil
  end

  # An ice_cube schedule anchored at `dtstart`, or nil when there's no usable rule.
  def schedule(dtstart)
    r = rule
    return nil unless r && dtstart

    IceCube::Schedule.new(dtstart).tap { |s| s.add_recurrence_rule(r) }
  end

  # The next occurrence strictly after `after`, given the series start `dtstart`.
  # nil when non-recurring or the series has ended (COUNT/UNTIL exhausted).
  def next_occurrence(dtstart:, after: Time.current)
    schedule(dtstart)&.next_occurrence(after)
  end

  # Occurrences within the inclusive [from, to] window, given the series start.
  # Capped at MAX_OCCURRENCES.
  def occurrences_between(dtstart:, from:, to:, limit: MAX_OCCURRENCES)
    sched = schedule(dtstart)
    return [] unless sched

    sched.occurrences_between(from, to).first(limit)
  end

  def to_s
    rrule.to_s
  end
end
