# frozen_string_literal: true

module People
  # Ranks the People list: who needs you first, then everyone else — by how much
  # the relationship matters and how live it is, never by raw age or raw recency.
  # (Raw age is how an unanswered newsletter from March ends up at the top.)
  #
  # Deterministic and query-free: it scores a Facts record the caller builds from
  # data the page already loads — the person's inbox threads, their contacts, the
  # newest inbound message, and their People::Standing. No new AI at rank time;
  # the AI reads it consults (action prompt, priority, follow-up) are already on
  # the rows. Same vocabulary as Feed::Ranking and Emails::SkimBuilder: a starred
  # sender tops, client/partner/colleague are VIP, an engaged thread is one you
  # wrote in.
  #
  # Strength (0..~9) — evidence of a real relationship: threads with mail both
  # ways (dominant), threads you wrote in at all, mail volume, an explicit star,
  # a whitelist, a sender-kind verdict (vs the never-classified default), and the
  # labelled relationship. Every term saturates, so no single signal runs away.
  #
  # Need you = kind × (0.5 + wait) × (1 + strength) + bonuses
  #   `wait` saturates over the first week: between two genuine asks from equally
  #   important people the one waiting longer comes first, but an ask from someone
  #   you barely know never outranks one from a real correspondent. A reply you
  #   owe outranks a nudge you could send. Which asks are genuine is decided
  #   upstream by People::Standing (a human message, addressed to you, not stale).
  # Recent   = recency × (1 + ½ × strength) + bonuses
  #   Recency-led (14-day half-life), relationship-tiebroken: a week-old regular
  #   beats yesterday's one-off; a month-old regular does not.
  class Priority
    HALF_LIFE_DAYS = 14.0
    WAIT_SATURATION_DAYS = 7.0

    # Mirrors Emails::SkimBuilder#vip? / Feed::Ranking's relationship boost.
    VIP_RELATIONSHIPS = %w[client partner colleague].freeze
    KNOWN_RELATIONSHIPS = (Person::RELATIONSHIP_TYPES - VIP_RELATIONSHIPS - %w[self unknown]).freeze

    STRENGTH_WEIGHTS = {
      two_way: 3.0,     # threads with mail both ways — you answered them
      outbound: 1.0,    # threads you wrote in, replies or cold sends
      volume: 1.0,      # mail received from them, saturating around 30
      starred: 2.0,     # you starred them
      allowed: 0.5,     # you whitelisted them
      classified: 0.5,  # a sender-kind verdict exists (heuristic or taught)
      vip: 1.0,         # Scout labelled them client / partner / colleague
      known: 0.25       # …or vendor / service provider / personal
    }.freeze
    THREAD_SATURATION = 2.0    # one real exchange already counts; three is a relationship
    VOLUME_SATURATION = 10.0

    KIND_WEIGHTS = { you_owe: 1.0, nudge: 0.7 }.freeze
    RECENT_STRENGTH_WEIGHT = 0.5
    PROMPT_BONUS = 0.5       # the latest message carries a Scout action prompt
    IMPORTANT_BONUS = 0.5    # …or the AI read it as high priority / important
    FOLLOW_UP_BONUS = 0.5    # an AI-confirmed follow-up has come due

    Facts = Data.define(
      :standing,           # People::Standing::Result
      :two_way_threads,    # threads with both an inbound and an outbound
      :outbound_threads,   # threads you wrote in (includes two-way)
      :email_count,        # mail received from the counterpart's contacts
      :starred,
      :allowed,
      :classified,         # any contact carries a sender_kind verdict
      :relationship_type,  # Person::RELATIONSHIP_TYPES value or nil
      :action_prompt,      # latest inbound carries a Scout action prompt
      :important,          # latest inbound is high priority / important
      :follow_up_due,      # a thread's AI-confirmed follow-up has come due
      :last_activity       # Time or nil
    )

    Score = Data.define(:value, :needs_you, :strength, :recency)

    class << self
      def score(facts, now: Time.current) = new(facts, now: now).score

      # Builds the Facts for one person from already-loaded records: their inbox
      # threads (reply-state columns), their contacts, and the newest inbound
      # message. Pure over the records handed in — no queries.
      def facts_for(standing:, threads:, contacts:, latest_inbound:, relationship_type:, last_activity:, now: Time.current)
        Facts.new(
          standing: standing,
          two_way_threads: threads.count { |t| t.last_inbound_at.present? && t.last_outbound_at.present? },
          outbound_threads: threads.count { |t| t.last_outbound_at.present? },
          email_count: contacts.sum { |c| c.email_count.to_i },
          starred: contacts.any?(&:starred?),
          allowed: contacts.any?(&:allowed?),
          classified: contacts.any? { |c| c.sender_kind_source.present? },
          relationship_type: relationship_type.presence,
          action_prompt: latest_inbound&.ai_action_prompt.to_s.strip.present?,
          important: important?(latest_inbound),
          follow_up_due: threads.any? { |t| t.follow_up_due?(now) },
          last_activity: last_activity
        )
      end

      # Folds several people's facts (an organization's members) into one: the
      # relationship evidence adds up, the liveliest thread wins, and the
      # standing is the one the caller composed for the group.
      def merge_facts(facts_list, standing:)
        return nil if facts_list.empty?

        Facts.new(
          standing: standing,
          two_way_threads: facts_list.sum(&:two_way_threads),
          outbound_threads: facts_list.sum(&:outbound_threads),
          email_count: facts_list.sum(&:email_count),
          starred: facts_list.any?(&:starred),
          allowed: facts_list.any?(&:allowed),
          classified: facts_list.any?(&:classified),
          relationship_type: facts_list.map(&:relationship_type).find { |r| VIP_RELATIONSHIPS.include?(r) } ||
                             facts_list.filter_map(&:relationship_type).first,
          action_prompt: facts_list.any?(&:action_prompt),
          important: facts_list.any?(&:important),
          follow_up_due: facts_list.any?(&:follow_up_due),
          last_activity: facts_list.filter_map(&:last_activity).max
        )
      end

      private

      def important?(message)
        return false if message.nil?

        message.ai_priority.to_s == "high" || message.category.to_s == "important"
      end
    end

    def initialize(facts, now: Time.current)
      @facts = facts
      @now = now
    end

    def score
      Score.new(
        value: (needs_you? ? obligation_score : recent_score).round(4),
        needs_you: needs_you?,
        strength: strength.round(4),
        recency: recency.round(4)
      )
    end

    # How much this counterpart matters, 0..~9.
    def strength
      w = STRENGTH_WEIGHTS
      w[:two_way] * saturate(@facts.two_way_threads, THREAD_SATURATION) +
        w[:outbound] * saturate(@facts.outbound_threads, THREAD_SATURATION) +
        w[:volume] * saturate(@facts.email_count, VOLUME_SATURATION) +
        (@facts.starred ? w[:starred] : 0) +
        (@facts.allowed ? w[:allowed] : 0) +
        (@facts.classified ? w[:classified] : 0) +
        relationship_weight
    end

    # 1.0 now, halving every HALF_LIFE_DAYS; 0 when there was never any mail.
    def recency
      return 0.0 if @facts.last_activity.blank?

      Math.exp(-Math.log(2) * days_since(@facts.last_activity) / HALF_LIFE_DAYS)
    end

    private

    def needs_you? = @facts.standing.needs_you

    def obligation_score
      kind = KIND_WEIGHTS.fetch(@facts.standing.kind, KIND_WEIGHTS[:nudge])
      wait = saturate(@facts.standing.overdue_days, WAIT_SATURATION_DAYS)
      kind * (0.5 + wait) * (1 + strength) + bonuses
    end

    def recent_score
      recency * (1 + RECENT_STRENGTH_WEIGHT * strength) + bonuses
    end

    def bonuses
      (@facts.action_prompt ? PROMPT_BONUS : 0) +
        (@facts.important ? IMPORTANT_BONUS : 0) +
        (@facts.follow_up_due ? FOLLOW_UP_BONUS : 0)
    end

    def relationship_weight
      type = @facts.relationship_type.to_s
      return STRENGTH_WEIGHTS[:vip] if VIP_RELATIONSHIPS.include?(type)
      return STRENGTH_WEIGHTS[:known] if KNOWN_RELATIONSHIPS.include?(type)

      0
    end

    # 0 → 0, rising towards 1; `scale` is where it reaches ~63%.
    def saturate(count, scale)
      1 - Math.exp(-count.to_f / scale)
    end

    def days_since(time)
      [ (@now - time) / 1.day, 0 ].max
    end
  end
end
