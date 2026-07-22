module Commitments
  # Value object representing a single temporal neighbor of a candidate commitment.
  # `kind` is "reminder", "task", or "calendar_event"; `record` is the AR model.
  Neighbor = Struct.new(:kind, :record, keyword_init: true)

  # Finds existing commitments (reminders, tasks, and calendar events) whose date
  # falls within a symmetric window around the candidate's `due_at`. Used by both
  # builders to decide whether to invoke the LLM novelty gate, and by the matcher
  # itself as the comparison set.
  #
  # Pure query object — no LLM calls, no side effects.
  module Neighbors
    module_function

    # Returns an Array<Neighbor> sorted by absolute temporal distance from `due_at`
    # within each kind, capped at `per_kind` per kind before merging. Returns []
    # when `due_at` is nil.
    def around(workspace:, due_at:, window: 1.day, per_kind: 8)
      return [] if due_at.nil?

      window_start = due_at - window
      window_end   = due_at + window

      reminders_near(workspace, due_at, window_start, window_end, per_kind) +
        tasks_near(workspace, due_at, window_start, window_end, per_kind) +
        events_near(workspace, due_at, window_start, window_end, per_kind)
    end

    def reminders_near(workspace, due_at, window_start, window_end, per_kind)
      # ALL statuses — dismissed reminders suppress re-staging just as much as live
      # ones (the builder handles the dismissed branch; here we just surface them).
      Reminder
        .where(workspace: workspace)
        .where("due_at::date BETWEEN ?::date AND ?::date", window_start, window_end)
        .order(distance_order("due_at", due_at))
        .limit(per_kind)
        .map { |r| Neighbor.new(kind: "reminder", record: r) }
    end

    def tasks_near(workspace, due_at, window_start, window_end, per_kind)
      # ALL statuses; dateless tasks excluded (no temporal anchor to compare against).
      Task
        .where(workspace: workspace)
        .where.not(due_at: nil)
        .where("due_at::date BETWEEN ?::date AND ?::date", window_start, window_end)
        .order(distance_order("due_at", due_at))
        .limit(per_kind)
        .map { |t| Neighbor.new(kind: "task", record: t) }
    end

    def events_near(workspace, due_at, window_start, window_end, per_kind)
      # Non-cancelled workspace events (a cancelled event shouldn't block staging).
      CalendarEvent
        .joins(calendar: :calendar_account)
        .where(calendar_accounts: { workspace_id: workspace.id })
        .visible
        .where("start_at::date BETWEEN ?::date AND ?::date", window_start, window_end)
        .order(distance_order("start_at", due_at))
        .limit(per_kind)
        .map { |e| Neighbor.new(kind: "calendar_event", record: e) }
    end

    # ORDER BY absolute distance from the candidate's timestamp. `column` is one of
    # this module's own literals ("due_at"/"start_at"), never caller input; the
    # timestamp is bound through sanitize_sql_array rather than interpolated.
    def distance_order(column, due_at)
      Arel.sql(
        ActiveRecord::Base.sanitize_sql_array(
          [ "ABS(EXTRACT(EPOCH FROM (#{column} - ?::timestamptz)))", due_at.utc.iso8601 ]
        )
      )
    end
  end
end
