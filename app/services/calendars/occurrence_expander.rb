# frozen_string_literal: true

module Calendars
  # Expands recurring "master" rows (those carrying an rrule) into the concrete
  # occurrences shown on the calendar for a given window, and merges them with the
  # already-loaded concrete events — letting a real synced instance win over an
  # expanded "ghost" whenever the provider has materialized that occurrence.
  #
  # Why both shapes exist: an event we just created (and any Zoho series) lives as
  # a single master row with an rrule; Google, pulled with singleEvents=true, later
  # syncs back concrete dated instances of the same series (tagged with the
  # master's id in recurring_event_provider_id, rrule blank). Expanding the master
  # locally makes the series show immediately and lets Zoho — which never expands
  # server-side — render at all, while the dedup makes the authoritative synced
  # instance win the moment it arrives (so a moved or cancelled occurrence is
  # respected instead of being drawn twice).
  #
  # Our presets produce at most one occurrence per day per series, so a ghost is
  # matched to a concrete instance by (series id, date) — immune to the intra-day
  # clock drift a DST change can introduce between local expansion and the
  # provider's instance times.
  class OccurrenceExpander
    def initialize(concrete:, masters:, from:, to:)
      @concrete = concrete.to_a
      @masters  = masters.to_a
      @from     = from
      @to       = to
    end

    # The full render list for [from, to]: the concrete events plus a ghost per
    # master occurrence not already covered by a concrete instance, start-ordered.
    def events
      (@concrete + ghosts).sort_by { |e| [ e.start_at, e.all_day ? 0 : 1 ] }
    end

    private

    # series id => Set of occurrence dates a concrete instance already covers.
    def covered_dates
      @covered_dates ||= @concrete.each_with_object(Hash.new { |h, k| h[k] = Set.new }) do |e, acc|
        next unless e.recurring_event_provider_id && e.start_at

        acc[e.recurring_event_provider_id] << e.start_at.to_date
      end
    end

    def ghosts
      @masters.flat_map { |master| ghosts_for(master) }
    end

    def ghosts_for(master)
      return [] unless master.start_at && master.recurrence.recurring?

      seen     = covered_dates[master.provider_event_id]
      duration = master.duration

      master.recurrence.occurrences_between(dtstart: master.start_at, from: @from, to: @to).filter_map do |occ|
        next if seen.include?(occ.to_date)

        build_ghost(master, occ, duration)
      end
    end

    # A transient copy of the master shifted to this occurrence's slot. It keeps the
    # master's id (so a click opens the series to edit), preloads calendar/type to
    # dodge N+1s, and is flagged + frozen so nothing treats it as a real instance.
    def build_ghost(master, occ, duration)
      ghost = master.dup
      ghost.id               = master.id
      ghost.start_at         = occ
      ghost.end_at           = duration ? occ + duration : nil
      ghost.calendar         = master.calendar
      ghost.event_type       = master.event_type
      ghost.occurrence_ghost = true
      ghost.readonly!
      ghost
    end
  end
end
