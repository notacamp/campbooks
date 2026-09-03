# frozen_string_literal: true

module Now
  # Scout's opening ledger on the Now page: an audit-safe tally of what Scout
  # actually did for this user in the last window, read straight off the workspace
  # Event log. It only ever states what the events prove (docs/messaging.md's
  # honesty rule) — never invents counts, never claims it "handled it all". SYSTEM
  # events only (actor_id IS NULL — Scout/automation, not a person), accessible to
  # the user, in the window. The copy is assembled in the view/component from
  # #buckets + #need_you; this object just does the honest counting.
  class Ledger
    # event name -> ledger bucket. Only the actions the ledger speaks about; other
    # system events still show in Scout's log, they just don't roll into a bucket.
    BUCKET_FOR = {
      "email.archived"         => :archived,
      "email.bulk_archived"    => :archived,
      "email.tagged"           => :tagged,
      "document.processed"     => :filed,
      "reminder.created"       => :reminders,
      "task.created"           => :tasks,
      "calendar_event.created" => :events
    }.freeze

    # Stable tie-break order when two buckets share a count.
    ORDER = %i[archived tagged filed reminders tasks events].freeze

    MAX_BUCKETS = 3

    # @param user [User]
    # @param since [ActiveSupport::TimeWithZone] window start (default 24h ago)
    # @param need_you [Integer] the deck's total count — "N need you"
    def initialize(user, since: 24.hours.ago, need_you: 0)
      @user = user
      @since = since
      @need_you = need_you.to_i
    end

    # The top buckets, largest first, capped — e.g. [{ key: :archived, count: 24 }].
    def buckets
      @buckets ||= tally
        .reject { |_key, count| count.zero? }
        .sort_by { |key, count| [ -count, ORDER.index(key) || ORDER.size ] }
        .first(MAX_BUCKETS)
        .map { |key, count| { key: key, count: count } }
    end

    # Every counted action across all buckets (not just the shown top three).
    def total
      tally.values.sum
    end

    def need_you
      @need_you
    end

    def any?
      buckets.any?
    end

    # True only when EVERY counted archive carries a rule/automation marker in its
    # payload — so "archived N emails by your rules" is never claimed unless the
    # log proves a rule drove all of them.
    def archived_by_rules?
      compute!
      @archived_total.positive? && @archived_rule_hits == @archived_total
    end

    private

    def tally
      compute!
      @tally
    end

    def compute!
      return if @tally

      @tally = Hash.new(0)
      @archived_total = 0
      @archived_rule_hits = 0

      system_events.each do |name, payload|
        bucket = BUCKET_FOR[name]
        next unless bucket

        payload = payload.is_a?(Hash) ? payload : {}
        case name
        when "email.bulk_archived"
          count = payload["count"].to_i
          count = 1 if count <= 0
          @tally[bucket] += count
          @archived_total += count
          @archived_rule_hits += count if rule_attributed?(payload)
        when "email.archived"
          @tally[bucket] += 1
          @archived_total += 1
          @archived_rule_hits += 1 if rule_attributed?(payload)
        when "task.created"
          # Only AI-suggested tasks are "suggested by Scout"; a manually created
          # task isn't Scout's doing.
          @tally[bucket] += 1 if truthy?(payload["ai_suggested"])
        else
          @tally[bucket] += 1
        end
      end
    end

    # name + payload for the window's SYSTEM events, in one query.
    def system_events
      Now::SystemEvents.scope(@user, since: @since).pluck(:name, :payload)
    end

    # The Events::Registry doesn't declare a rule key (email.archived's payload_keys
    # are just subject/from), but a rule-run may stamp one on the raw payload — so
    # we look there, and only claim a rule when it's proven.
    def rule_attributed?(payload)
      payload["rule_id"].present? ||
        payload["rule_name"].present? ||
        %w[rule rules automation].include?(payload["source"].to_s)
    end

    def truthy?(value)
      ActiveModel::Type::Boolean.new.cast(value) == true
    end
  end
end
