# frozen_string_literal: true

module Today
  # Assembles the Today surface payload: a ranked, finite "needs you" worklist
  # merged from People (PeopleStanding verb lanes), Money (reconciliation /
  # loan alerts, feature-gated), and Time (open asks + overdue deadlines). Also
  # builds the near-future coming_up list and Scout's handled-this-week counts.
  #
  # READ-ONLY. No mutations. Item actions are dispatched by the client to the
  # existing per-surface endpoints (POST /api/app/people/:id/action, etc.).
  class Aggregator
    # Verb sort priority: overdue/pay/decide before communication lanes.
    VERB_RANK = {
      "pay"    => 1,
      "decide" => 2,
      "chase"  => 3,
      "reply"  => 4,
      "nudge"  => 5,
      "do"     => 6
    }.freeze

    COMING_UP_DAYS  = 3
    COMING_UP_LIMIT = 5

    def initialize(user, workspace)
      @user      = user
      @workspace = workspace
      @zone      = user.effective_time_zone
      @now       = ::Time.current
    end

    # Returns { needs_you:, coming_up:, handled: } — plain hashes ready for
    # the serializer to wrap into the response envelope.
    def call
      {
        needs_you:  build_needs_you,
        coming_up:  build_coming_up,
        handled:    build_handled
      }
    end

    private

    # ── Needs you ──────────────────────────────────────────────────────────────

    def build_needs_you
      items = people_items + time_items
      items += money_items if money_enabled_and_entitled?
      rank_items(items)
    end

    # People: PeopleStanding rows where needs_you is true. The standings table
    # is refreshed (or a background job enqueued) before reading, matching the
    # DirectoryController pattern.
    def people_items
      ensure_standings_fresh
      rows = PeopleStanding.for_user(@user).needing.ranked
      rows.map { |row| build_people_item(row) }
    end

    # Time: overdue + upcoming tasks (next 7 days) and undated asks. Separate
    # call to Time::Agenda so the coming_up window can differ.
    def time_items
      today = @now.in_time_zone(@zone).to_date
      from  = @zone.local(today.year, today.month, today.day).beginning_of_day
      to    = from + 7.days

      agenda  = ::Time::Agenda.for(@user, from: from, to: to)
      undated = ::Time::Agenda.undated_for(@user)

      (agenda + undated)
        .select { |item| %i[task deadline].include?(item.kind) }
        .filter_map { |item| build_time_item(item) }
    end

    # Money: Money::Page#needs_you items, gated by Features.accounting? and the
    # workspace :accounting entitlement. Skip silently (not 403) so the Today
    # endpoint always succeeds regardless of plan.
    def money_items
      page = ::Money::Page.for(@workspace, @user, today: ::Date.current)
      page.needs_you.each_with_index.filter_map { |item, idx| build_money_item(item, idx) }
    rescue StandardError
      []
    end

    def money_enabled_and_entitled?
      Features.accounting? && @workspace.entitlements.feature?(:accounting)
    end

    # ── Item builders ──────────────────────────────────────────────────────────

    def build_people_item(row)
      verb = row.verb.to_s.presence || "reply"

      {
        id:       "people_#{row.counterpart_id}",
        source:   "people",
        ref_id:   row.counterpart_id,
        verb:     verb,
        place:    "Inbox",
        title:    row.name,
        subtitle: row.subtitle,
        read:     standing_line(row),
        due:      nil,
        draft:    nil,
        overdue:  false,
        actions:  people_actions(verb, row.counterpart_id)
      }
    end

    def build_time_item(item)
      record = item.record
      return nil unless record

      case item.kind
      when :task
        {
          id:       "time_task_#{record.id}",
          source:   "time",
          ref_id:   record.id,
          verb:     "do",
          place:    "Time",
          title:    item.title,
          subtitle: item.source_label,
          read:     item.why,
          due:      item.at&.iso8601 || item.day&.iso8601,
          draft:    nil,
          overdue:  item.overdue,
          actions:  task_actions(record.id)
        }
      when :deadline
        {
          id:       "time_deadline_#{record.id}",
          source:   "time",
          ref_id:   record.id,
          verb:     "decide",
          place:    "Time",
          title:    item.title,
          subtitle: item.source_label,
          read:     item.why,
          due:      item.at&.iso8601 || item.day&.iso8601,
          draft:    nil,
          overdue:  item.overdue,
          actions:  deadline_actions(record.id)
        }
      end
    end

    def build_money_item(item, idx)
      verb   = money_verb(item)
      ref_id = (item.transaction&.id || item.payload&.dig(:loan)&.id || "#{item.kind}_#{idx}").to_s

      {
        id:       "money_#{idx}_#{item.kind}",
        source:   "money",
        ref_id:   ref_id,
        verb:     verb,
        place:    "Money",
        title:    item.title,
        subtitle: Array(item.meta).filter_map { |m| m.is_a?(Hash) ? nil : m.to_s.presence }.join(" · "),
        read:     item.title,
        due:      nil,
        draft:    nil,
        overdue:  false,
        actions:  money_actions(item)
      }
    end

    # ── Actions ────────────────────────────────────────────────────────────────

    def people_actions(verb, ref_id)
      primary_kind = case verb
      when "reply"  then "reply"
      when "pay"    then "paid"
      when "decide" then "archive"
      when "chase"  then "reply"
      when "nudge"  then "reply"
      when "do"     then "done"
      else               "done"
      end

      primary_label = case verb
      when "chase"  then "Follow up"
      when "nudge"  then "Nudge"
      when "decide" then "Archive"
      when "pay"    then "Mark paid"
      when "do"     then "Done"
      when "reply"  then "Reply"
      else               "Done"
      end

      primary = { kind: primary_kind, label: primary_label, primary: true }
                  .merge(Today::ActionRoutes.people(primary_kind, ref_id))
      snooze  = { kind: "snooze", label: "Later", primary: false }
                  .merge(Today::ActionRoutes.people("snooze", ref_id))

      [ primary, snooze ]
    end

    def task_actions(task_id)
      [
        { kind: "done",   label: "Done",  primary: true }
            .merge(Today::ActionRoutes.ask("done", task_id)),
        { kind: "snooze", label: "Later", primary: false }
            .merge(Today::ActionRoutes.ask("snooze", task_id))
      ]
    end

    def deadline_actions(reminder_id)
      [
        { kind: "confirm", label: "Confirm", primary: true }
            .merge(Today::ActionRoutes.reminder("confirm", reminder_id)),
        { kind: "dismiss", label: "Dismiss", primary: false }
            .merge(Today::ActionRoutes.reminder("dismiss", reminder_id))
      ]
    end

    def money_actions(item)
      tx_id    = item.transaction&.id
      recon_id = item.transaction&.reconciliation_id

      case item.kind.to_s
      when /^loan/
        [ { kind: "view", label: "View loan", primary: true }
              .merge(Today::ActionRoutes.money("view")) ]
      when "add_statement"
        [ { kind: "add_statement", label: "Add statement", primary: true }
              .merge(Today::ActionRoutes.money("add_statement")) ]
      when "reconcile_statements"
        [ { kind: "reconcile", label: "Reconcile", primary: true }
              .merge(Today::ActionRoutes.money("reconcile")) ]
      else
        [ { kind: "review", label: "Review", primary: true }
              .merge(Today::ActionRoutes.money("review", tx_id: tx_id, recon_id: recon_id)) ]
      end
    end

    def money_verb(item)
      case item.kind.to_s
      when /^loan_missed/, "add_statement"
        "pay"
      else
        "decide"
      end
    end

    # ── Ranking ────────────────────────────────────────────────────────────────

    def rank_items(items)
      items.sort_by do |item|
        verb_rank = VERB_RANK.fetch(item[:verb].to_s, 99)
        overdue   = item[:overdue] ? 0 : 1
        due_str   = item[:due] || "9999-99-99T99:99:99Z"
        [ verb_rank, overdue, due_str ]
      end
    end

    # ── Coming up ──────────────────────────────────────────────────────────────

    def build_coming_up
      from = @now
      to   = @now + COMING_UP_DAYS.days

      ::Time::Agenda.for(@user, from: from, to: to)
        .select { |i| %i[event deadline].include?(i.kind) && !i.overdue }
        .first(COMING_UP_LIMIT)
        .map { |i| build_coming_up_item(i) }
    rescue StandardError
      []
    end

    def build_coming_up_item(item)
      {
        on:    item.at&.iso8601 || item.day&.iso8601,
        label: item.title,
        sub:   item.source_label,
        place: item.kind == :event ? "Calendar" : "Time"
      }
    end

    # ── Handled ────────────────────────────────────────────────────────────────

    # Counts of what Scout handled since the start of this calendar week, read
    # straight from the Event log (SYSTEM events only — actor_id IS NULL). Maps
    # the Now::Ledger BUCKET_FOR entries to the four Today fields:
    #   filed   = documents processed  (document.processed)
    #   matched = emails archived       (email.archived / email.bulk_archived)
    #   tucked  = emails tagged         (email.tagged)
    #   added   = tasks + reminders + calendar events created by Scout
    def build_handled
      cutoff = handled_since_cutoff
      events = Now::SystemEvents.scope(@user, since: cutoff).pluck(:name, :payload)

      filed   = events.count { |name, _| name == "document.processed" }
      matched = events.sum { |name, payload|
        case name
        when "email.bulk_archived"
          count = payload.is_a?(Hash) ? payload["count"].to_i : 0
          [ count, 1 ].max
        when "email.archived" then 1
        else 0
        end
      }
      tucked = events.count { |name, _| name == "email.tagged" }
      added  = events.count { |name, _| %w[task.created reminder.created calendar_event.created].include?(name) }

      {
        filed:   filed,
        matched: matched,
        tucked:  tucked,
        added:   added,
        since:   cutoff.to_date.iso8601
      }
    end

    def handled_since_cutoff
      @now.in_time_zone(@zone).beginning_of_week(:monday).beginning_of_day
    end

    # ── Helpers ────────────────────────────────────────────────────────────────

    def ensure_standings_fresh
      if People::Standings.missing?(@user)
        if Contact.where(workspace_id: @workspace.id).where("email_count > 0").exists?
          People::Standings.refresh!(@user)
        end
      elsif People::Standings.stale?(@user)
        People::StandingsRefreshJob.enqueue_for(@user.id)
      end
    end

    def standing_line(row)
      People::StandCopy.line(row.standing)
    rescue StandardError
      nil
    end
  end
end
