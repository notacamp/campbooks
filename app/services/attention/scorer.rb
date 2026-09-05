# frozen_string_literal: true

module Attention
  # Turns one counterpart's Facts into a weight. Additive saturating terms (no
  # single signal can run away), squashed to 0..1, shrunk toward a neutral prior
  # while evidence is thin, then clamped by the explicit signals the user gave
  # (a star, a block, a taught verdict) which always win over inference.
  class Scorer
    Result = Data.define(:weight, :confidence, :raw, :reasons)

    PRIOR = 0.3
    SERVICE_PRIOR = 0.15
    REPLY_WINDOW_DAYS = 14

    WEIGHTS = {
      replies: 2.5, speed: 1.5, two_way: 1.5, outbound: 0.5, volume: 0.5, opens: 0.75,
      meetings: 1.0, money: 1.0, engaged: 0.75,
      ignored: -2.0, dismissed: -1.5, service: -1.5,
      starred: 3.0, allowed: 1.0, vip: 1.5, known: 0.5, urgency_high: 0.5, urgency_low: -0.5,
      taught_important: 4.0, taught_unimportant: -4.0
    }.freeze
    SATURATION = { replies: 3.0, speed_hours: 48.0, two_way: 2.0, outbound: 2.0, volume: 10.0,
                   meetings: 2.0, money: 2.0, engaged: 3.0, evidence: 6.0 }.freeze

    # Logistic squash: raw 2.5 -> 0.5; raw 0 -> ~0.12; raw 6 -> ~0.94.
    SQUASH_MIDPOINT = 2.5
    SQUASH_SCALE = 1.25

    FLOORS   = { taught_important: 0.9, starred: 0.8, vip: 0.55, allowed: 0.45 }.freeze
    CEILINGS = { taught_unimportant: 0.1 }.freeze
    EXPLICIT_CONFIDENCE_FLOOR = 0.6

    VIP_RELATIONSHIPS   = %w[client partner colleague].freeze
    KNOWN_RELATIONSHIPS = %w[vendor service_provider personal].freeze
    MIN_INBOUND_FOR_RATES = 3

    def self.score(facts) = new(facts).score

    def initialize(facts)
      @f = facts
    end

    def score
      # Blocked always wins immediately.
      if @f.blocked
        return Result.new(
          weight: 0.0,
          confidence: 1.0,
          raw: raw_sum,
          reasons: [ Reason.new(key: "blocked") ]
        )
      end

      raw = raw_sum
      squashed = logistic(raw)

      evidence_pts = evidence_points
      conf = sat(evidence_pts, SATURATION[:evidence]).round(4)

      prior = @f.sender_kind == "service" ? SERVICE_PRIOR : PRIOR
      w = conf * squashed + (1 - conf) * prior

      # Explicit clamps: ceiling first, then floors.
      explicit_applied = false

      # Apply floors first (in priority order: strongest wins).
      if @f.taught == "important" && w < FLOORS[:taught_important]
        w = FLOORS[:taught_important]
        explicit_applied = true
      end

      if @f.starred && w < FLOORS[:starred]
        w = FLOORS[:starred]
        explicit_applied = true
      end

      if vip? && w < FLOORS[:vip]
        w = FLOORS[:vip]
        explicit_applied = true
      end

      if @f.allowed && w < FLOORS[:allowed]
        w = FLOORS[:allowed]
        explicit_applied = true
      end

      # Apply the unimportant ceiling LAST so the user's taught verdict always wins,
      # even over a starred floor.
      if @f.taught == "unimportant"
        w = [ w, CEILINGS[:taught_unimportant] ].min
        explicit_applied = true
      end

      conf = [ conf, EXPLICIT_CONFIDENCE_FLOOR ].max if explicit_applied

      w = w.round(4).clamp(0.0, 1.0)

      Result.new(
        weight: w,
        confidence: conf,
        raw: raw,
        reasons: build_reasons(conf)
      )
    end

    # { signal_name => Float contribution } — exposed for specs and tuning.
    def terms
      t = {}
      f = @f
      inbound = f.inbound_count

      t[:replies]  = WEIGHTS[:replies] * sat(f.replied_count, SATURATION[:replies])

      if f.replied_count > 0 && f.median_reply_hours
        t[:speed] = WEIGHTS[:speed] * (1 - sat(f.median_reply_hours, SATURATION[:speed_hours]))
      else
        t[:speed] = 0.0
      end

      t[:two_way]  = WEIGHTS[:two_way] * sat(f.two_way_threads, SATURATION[:two_way])
      t[:outbound] = WEIGHTS[:outbound] * sat(f.outbound_threads, SATURATION[:outbound])

      if inbound >= MIN_INBOUND_FOR_RATES
        t[:volume] = WEIGHTS[:volume] * sat(f.addressed_count, SATURATION[:volume])
        t[:opens]  = WEIGHTS[:opens] * (f.opened_count.to_f / inbound)
        ignore_rate = ((f.archived_unread_count + f.trashed_count + f.skim_archive_count).to_f / inbound).clamp(0, 1)
        t[:ignored] = WEIGHTS[:ignored] * ignore_rate
      else
        t[:volume] = 0.0
        t[:opens]  = 0.0
        t[:ignored] = 0.0
      end

      t[:meetings] = WEIGHTS[:meetings] * sat(f.meetings_count, SATURATION[:meetings])
      t[:money]    = WEIGHTS[:money] * sat(f.invoices_count, SATURATION[:money])

      engaged_sum = f.snoozed_count + f.forwarded_count + f.tagged_count + f.feed_acted_count
      t[:engaged]  = WEIGHTS[:engaged] * sat(engaged_sum, SATURATION[:engaged])

      denom = f.feed_dismissed_count + f.feed_acted_count
      if denom >= 2
        t[:dismissed] = WEIGHTS[:dismissed] * (f.feed_dismissed_count.to_f / denom)
      else
        t[:dismissed] = 0.0
      end

      t[:service]  = f.sender_kind == "service" ? WEIGHTS[:service] : 0.0

      # Explicit additive terms.
      t[:starred]           = f.starred ? WEIGHTS[:starred] : 0.0
      t[:allowed]           = f.allowed ? WEIGHTS[:allowed] : 0.0
      t[:vip]               = vip? ? WEIGHTS[:vip] : 0.0
      t[:known]             = known? ? WEIGHTS[:known] : 0.0
      t[:urgency_high]      = f.urgency_level == "high" ? WEIGHTS[:urgency_high] : 0.0
      t[:urgency_low]       = f.urgency_level == "low" ? WEIGHTS[:urgency_low] : 0.0
      t[:taught_important]  = f.taught == "important" ? WEIGHTS[:taught_important] : 0.0
      t[:taught_unimportant] = f.taught == "unimportant" ? WEIGHTS[:taught_unimportant] : 0.0

      t
    end

    private

    def raw_sum = terms.values.sum

    def logistic(raw)
      1.0 / (1.0 + Math.exp(-(raw - SQUASH_MIDPOINT) / SQUASH_SCALE))
    end

    # sat(x, k) = 1 - exp(-x / k)
    def sat(x, k)
      1 - Math.exp(-x.to_f / k)
    end

    def vip?
      VIP_RELATIONSHIPS.include?(@f.relationship_type.to_s)
    end

    def known?
      KNOWN_RELATIONSHIPS.include?(@f.relationship_type.to_s)
    end

    def evidence_points
      f = @f
      pts = f.replied_count +
            f.opened_count / 2.0 +
            f.archived_unread_count +
            f.trashed_count +
            f.feed_acted_count +
            f.feed_dismissed_count +
            f.skim_archive_count +
            f.skim_keep_count +
            f.meetings_count +
            f.invoices_count +
            f.two_way_threads
      pts += 3 if f.starred
      pts += 6 if f.taught
      pts += 1 if vip? || known?
      pts
    end

    def build_reasons(confidence)
      f = @f
      inbound = f.inbound_count
      candidates = []

      if f.taught == "important"
        candidates << [ Reason.new(key: "taught_important"), WEIGHTS[:taught_important].abs ]
      end

      if f.taught == "unimportant"
        candidates << [ Reason.new(key: "taught_unimportant"), WEIGHTS[:taught_unimportant].abs ]
      end

      if f.blocked
        return [ Reason.new(key: "blocked") ]
      end

      if f.starred
        candidates << [ Reason.new(key: "starred"), WEIGHTS[:starred].abs ]
      end

      if f.allowed && !f.starred
        candidates << [ Reason.new(key: "allowed"), WEIGHTS[:allowed].abs ]
      end

      if vip?
        candidates << [ Reason.new(key: "vip"), WEIGHTS[:vip].abs ]
      end

      if f.urgency_level == "high"
        candidates << [ Reason.new(key: "urgent_sender"), WEIGHTS[:urgency_high].abs ]
      end

      if f.replied_count >= 2 && f.median_reply_hours && f.median_reply_hours <= 24
        mag = terms[:replies].abs + terms[:speed].abs
        candidates << [ Reason.new(key: "replies_fast", params: { hours: f.median_reply_hours.round }), mag ]
      elsif f.replied_count >= 1
        mag = terms[:replies].abs
        candidates << [ Reason.new(key: "replies", params: { count: f.replied_count, total: f.addressed_count }), mag ]
      end

      if f.two_way_threads >= 2
        candidates << [ Reason.new(key: "two_way", params: { count: f.two_way_threads }), terms[:two_way].abs ]
      end

      if f.meetings_count >= 1
        candidates << [ Reason.new(key: "meetings", params: { count: f.meetings_count }), terms[:meetings].abs ]
      end

      if f.invoices_count >= 1
        candidates << [ Reason.new(key: "invoices", params: { count: f.invoices_count }), terms[:money].abs ]
      end

      if f.settled_count >= 2 && f.median_settle_delay_days && f.median_settle_delay_days <= 0
        candidates << [ Reason.new(key: "pays_promptly"), WEIGHTS[:money].abs ]
      end

      engaged_total = f.snoozed_count + f.forwarded_count + f.tagged_count + f.feed_acted_count
      if engaged_total >= 2
        candidates << [ Reason.new(key: "engaged", params: { count: engaged_total }), terms[:engaged].abs ]
      end

      if inbound >= MIN_INBOUND_FOR_RATES
        ignore_rate = ((f.archived_unread_count + f.trashed_count + f.skim_archive_count).to_f / inbound).clamp(0, 1)
        if ignore_rate >= 0.5
          candidates << [ Reason.new(key: "ignored", params: { percent: (ignore_rate * 100).round }), terms[:ignored].abs ]
        end
      end

      if f.feed_dismissed_count >= 2
        candidates << [ Reason.new(key: "dismissed", params: { count: f.feed_dismissed_count }), terms[:dismissed].abs ]
      end

      if f.sender_kind == "service"
        candidates << [ Reason.new(key: "service"), WEIGHTS[:service].abs ]
      end

      # If no evidence at all, use the "new" reason.
      if confidence == 0.0 && !f.starred && !f.allowed && !f.blocked && f.taught.nil? && !vip?
        return [ Reason.new(key: "new") ]
      end

      candidates.sort_by { |_r, mag| -mag }.first(3).map(&:first)
    end
  end
end
