module Feed
  # The feed's single ranking brain. Sources score their own domain (how urgent
  # is this reminder / follow-up / action *as of now*, on a 0–100 scale); this
  # class turns that intrinsic score into the final materialized rank by mixing
  # in the cross-cutting signals no single source owns:
  #
  #   final = (intrinsic + relevance boosts − ignored penalty)
  #             × recency decay × engagement multiplier
  #
  # Relevance boosts (email subjects) read the sender and the conversation: a
  # starred or analyzed-relationship contact lifts a card, a sender whose mail
  # historically runs urgent lifts it further, provider noise categories
  # (promotions, social, …) sink it, and a thread the user has actually written
  # in outranks one they never touched.
  #
  # The behavioral signals close the loop on what this user does with the feed:
  # a card they've been shown for days without touching drifts down (ignored
  # penalty), and card kinds they habitually dismiss earn a per-kind discount
  # while kinds they act on earn a premium (engagement multiplier).
  #
  # Recency decay is what keeps the feed honest about time: an item's score
  # halves every HALF_LIFE days past its action moment (sort_at), so a two-year-
  # old follow-up decays to ~0 and sinks to the bottom instead of pinning above
  # this morning's mail. Future-dated items (a reminder due next week) don't
  # decay — their proximity urgency is already the source's score.
  #
  # The attention cluster is gated on the *final* score: a source can nominate a
  # card as urgent, but once decay drags it under ATTENTION_FLOOR it demotes to
  # the ranked timeline. Ranking must never take the feed down — any per-card
  # failure falls back to the source's own score.
  class Ranking
    # Halving period for the decayed score, per kind. Waiting-shaped kinds age
    # slower (a follow-up is still worth a nudge after a week); actionable mail
    # goes stale faster.
    HALF_LIFE_DAYS = {
      "follow_up" => 14.0,
      "reply_reminder" => 14.0,
      "task" => 14.0,
      "starred_email" => 10.0
    }.freeze
    DEFAULT_HALF_LIFE_DAYS = 7.0

    # Everything keeps full strength this long, so today's items rank purely on
    # urgency and relevance rather than shuffling by the hour.
    GRACE = 36.hours

    # Minimum final score to hold a spot in the pinned attention cluster.
    ATTENTION_FLOOR = 55

    STARRED_CONTACT_BOOST = 25
    KNOWN_RELATIONSHIP_BOOST = 10
    ENGAGED_THREAD_BOOST = 10
    BUSY_THREAD_BOOST = 5
    BUSY_THREAD_AT = 4
    IMPORTANT_CATEGORY_BOOST = 10
    NOISE_CATEGORY_PENALTY = 25
    # Contact analysis's read on how urgent a sender's mail tends to run.
    SENDER_URGENCY_BOOST = { "high" => 8, "low" => -5 }.freeze

    # A card the user was shown this long ago and never touched drifts down.
    SEEN_IGNORE_AFTER = 2.days
    SEEN_IGNORE_PENALTY = 8

    # Per-kind engagement: how far back to read the user's feed history, and the
    # Laplace prior that keeps the multiplier at exactly 1.0 with no evidence
    # and near it while evidence is thin.
    ENGAGEMENT_WINDOW = 90.days
    ENGAGEMENT_PRIOR = 2.0
    ENGAGEMENT_MULTIPLIER_RANGE = (0.7..1.2)

    # Mirrors the noise set Feed::Rewind excludes from highlights.
    NOISE_CATEGORIES = %w[promotions notifications social updates].freeze
    # relationship_type values that say nothing about relevance.
    UNINFORMATIVE_RELATIONSHIPS = %w[unknown self].freeze

    def initialize(user, now: Time.current)
      @user = user
      @now = now
    end

    # Rewrites :score and :attention on each [kind, candidate] pair in place.
    # Batch-loads the contact/thread signals once for the whole run.
    def apply!(pairs)
      preload(pairs.map { |_kind, c| c[:subject] })
      pairs.each do |kind, candidate|
        rank(kind, candidate)
      rescue => e
        # Keep the source's own score — a ranking hiccup must not drop the card.
        Rails.logger.warn("[Feed::Ranking] #{kind} failed: #{e.class}: #{e.message}")
      end
      pairs
    end

    private

    def rank(kind, candidate)
      base = candidate[:score].to_i + boosts(candidate) - seen_ignore_penalty(candidate)
      final = (base.clamp(0, nil) *
               decay(kind, candidate[:sort_at]) *
               engagement_multiplier(kind)).round
      candidate[:score] = final
      candidate[:attention] = !!candidate[:attention] && final >= ATTENTION_FLOOR
    end

    # The generator stamps each candidate with the seen_at of the row it already
    # has; shown for days with no reaction means "not now" — yield to fresher cards.
    def seen_ignore_penalty(candidate)
      seen_at = candidate[:seen_at]
      seen_at.present? && seen_at < @now - SEEN_IGNORE_AFTER ? SEEN_IGNORE_PENALTY : 0
    end

    # How this user treats this kind of card, from their trailing feed history:
    # acted = engagement, dismissed = rejection. Deliberately blind to expiries —
    # a calendar card expiring after the meeting says nothing about interest —
    # and to rows never resolved at all (most cards are simply scrolled past).
    def engagement_multiplier(kind)
      stats = engagement_stats[kind.to_s]
      return 1.0 unless stats

      rate = (stats[:acted] + ENGAGEMENT_PRIOR) /
             (stats[:acted] + stats[:dismissed] + ENGAGEMENT_PRIOR * 2)
      (0.7 + 0.6 * rate).clamp(ENGAGEMENT_MULTIPLIER_RANGE.begin, ENGAGEMENT_MULTIPLIER_RANGE.end)
    end

    def engagement_stats
      @engagement_stats ||= @user.feed_items
        .where(created_at: (@now - ENGAGEMENT_WINDOW)..)
        .group(:kind)
        .pluck(
          :kind,
          Arel.sql("COUNT(*) FILTER (WHERE acted_at IS NOT NULL)"),
          Arel.sql("COUNT(*) FILTER (WHERE dismissed_at IS NOT NULL)")
        )
        .to_h { |kind, acted, dismissed| [ kind, { acted: acted, dismissed: dismissed } ] }
    end

    # 1.0 while the action moment is in the future or within GRACE, then
    # exponential: halves every HALF_LIFE_DAYS past it.
    def decay(kind, sort_at)
      return 1.0 if sort_at.nil?

      age = @now - sort_at.to_time - GRACE.to_f
      return 1.0 if age <= 0

      half_life = HALF_LIFE_DAYS.fetch(kind, DEFAULT_HALF_LIFE_DAYS) * 1.day.to_f
      0.5**(age / half_life)
    end

    def boosts(candidate)
      subject = candidate[:subject]
      return 0 unless subject.is_a?(EmailMessage)

      contact = contacts_by_id[subject.contact_id]
      thread_count = candidate.dig(:data, "thread_count").to_i

      total = 0
      if contact
        total += STARRED_CONTACT_BOOST if contact[:starred]
        total += KNOWN_RELATIONSHIP_BOOST if contact[:known_relationship]
        total += SENDER_URGENCY_BOOST.fetch(contact[:urgency], 0)
      end
      total += ENGAGED_THREAD_BOOST if engaged_thread_ids.include?(subject.email_thread_id)
      total += BUSY_THREAD_BOOST if thread_count >= BUSY_THREAD_AT
      total + category_boost(subject)
    end

    # Provider/AI category verdict: important mail up, bulk noise down. Sources
    # select only the columns they need, so guard for the attribute.
    def category_boost(subject)
      return 0 unless subject.has_attribute?(:category)

      case subject.category
      when "important" then IMPORTANT_CATEGORY_BOOST
      when *NOISE_CATEGORIES then -NOISE_CATEGORY_PENALTY
      else 0
      end
    end

    def preload(subjects)
      messages = subjects.grep(EmailMessage)
      @contact_ids = messages.filter_map(&:contact_id).uniq
      @thread_ids = messages.filter_map(&:email_thread_id).uniq
      # Reset the memoized lookups so apply! is safely re-entrant.
      @contacts_by_id = nil
      @engaged_thread_ids = nil
    end

    def contacts_by_id
      @contacts_by_id ||= Contact.where(id: @contact_ids)
        .pluck(:id, :starred_at, :relationship_type, :communication_patterns)
        .to_h do |id, starred_at, relationship, patterns|
          known = relationship.present? && UNINFORMATIVE_RELATIONSHIPS.exclude?(relationship)
          urgency = patterns.is_a?(Hash) ? patterns["urgency_level"] : nil
          [ id, { starred: starred_at.present?, known_relationship: known, urgency: urgency } ]
        end
    end

    # Threads the user has written in — a conversation they're part of, not one
    # that merely happened at them.
    def engaged_thread_ids
      @engaged_thread_ids ||= EmailThread.where(id: @thread_ids)
        .where.not(last_outbound_at: nil).pluck(:id).to_set
    end
  end
end
