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
      "reply_owed" => 14.0,
      "task" => 14.0,
      "starred_email" => 10.0,
      "late_payable" => 7.0
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

    # Learned relevance: the counterpart's attention weight, centered on the neutral
    # prior so an unknown sender gets no lift and no penalty. 0.3 → 0, 0.9 → +36,
    # 0.02 → −17, 1.0 → +42 — the same band the legacy star/label boosts spanned.
    # Replaces the fixed star / known-relationship / sender-urgency boosts once the
    # user has any attention rows; a fresh workspace (no rows) keeps the legacy path.
    ATTENTION_PIVOT = Attention::Scorer::PRIOR
    ATTENTION_SPAN  = 60

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
    # Batch-loads the contact/thread/attention signals once for the whole run.
    def apply!(pairs)
      preload(pairs.map { |_kind, c| c[:subject] })
    rescue => e
      # A batch-level failure (e.g. attention weights unavailable) must never take
      # the feed down — leave every candidate on its source score, untouched.
      Rails.logger.warn("[Feed::Ranking] preload failed: #{e.class}: #{e.message}")
      pairs
    else
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
      # Record why this card ranks where it does (the top positive reason + weight),
      # so the card can print a kicker. Runs inside the per-candidate rescue, so a
      # stamping hiccup keeps the ranked score rather than dropping the card.
      stamp_attention(candidate)
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

    # Relevance boosts. The counterpart term is learned (the attention weight)
    # once the user has any rows; on a fresh workspace it falls back to the legacy
    # fixed star / known-relationship / sender-urgency boosts, so nothing changes
    # until Scout has watched the user. Thread and category boosts are email-only
    # and unchanged; the counterpart term now also lifts a Document (a late bill
    # from a weighted vendor) via its linked message's contact.
    def boosts(candidate)
      subject = candidate[:subject]
      contact_id = contact_id_for(subject)

      total = 0
      if @weights_missing
        # Legacy path: the fixed star / relationship / urgency boosts, read from the
        # contact's flags. Consulted for every email subject (as the pre-attention
        # code did) so a contact-lookup failure is caught per-candidate, not batch.
        total += legacy_contact_boosts(contact_id) if subject.is_a?(EmailMessage) || contact_id
      elsif contact_id
        total += attention_boost(weight_for(contact_id))
      end

      if subject.is_a?(EmailMessage)
        thread_count = candidate.dig(:data, "thread_count").to_i
        total += ENGAGED_THREAD_BOOST if engaged_thread_ids.include?(subject.email_thread_id)
        total += BUSY_THREAD_BOOST if thread_count >= BUSY_THREAD_AT
        total += category_boost(subject)
      end

      total
    end

    # The contact behind a subject: an email's sender, or (for a Document) the
    # contact of the message it arrived on. nil for reminders, tasks, events.
    def contact_id_for(subject)
      case subject
      when EmailMessage then subject.contact_id
      when Document then @doc_contact_ids[subject.id]
      end
    end

    # Today's fixed relevance block, kept for the no-attention-rows fallback.
    def legacy_contact_boosts(contact_id)
      contact = contacts_by_id[contact_id]
      return 0 unless contact

      total = 0
      total += STARRED_CONTACT_BOOST if contact[:starred]
      total += KNOWN_RELATIONSHIP_BOOST if contact[:known_relationship]
      total += SENDER_URGENCY_BOOST.fetch(contact[:urgency], 0)
      total
    end

    # Centered on the neutral prior so an unknown/absent row lifts nothing.
    def attention_boost(weight)
      ((weight - ATTENTION_PIVOT) * ATTENTION_SPAN).round
    end

    def weight_for(contact_id)
      @weights_by_contact[contact_id]&.weight || Attention::Weights::DEFAULT_WEIGHT
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
      messages  = subjects.grep(EmailMessage)
      documents = subjects.grep(Document)

      @thread_ids      = messages.filter_map(&:email_thread_id).uniq
      @doc_contact_ids = document_contact_ids(documents.map(&:id))

      # Both message senders and the vendors behind late-bill documents.
      @contact_ids = (messages.filter_map(&:contact_id) + @doc_contact_ids.values).uniq

      # Learned relevance: one weights lookup for the whole batch. Missing (no rows
      # for this user yet) keeps the legacy boosts and skips stamping entirely.
      @weights            = Attention::Weights.new(@user)
      @weights_missing    = @weights.missing?
      @weights_by_contact = @weights_missing ? {} : @weights.contacts(@contact_ids)

      # Reset the memoized lookups so apply! is safely re-entrant.
      @contacts_by_id = nil
      @engaged_thread_ids = nil
    end

    # { document_id => first non-nil linked-message contact_id } in one query.
    def document_contact_ids(doc_ids)
      return {} if doc_ids.blank?

      map = {}
      DocumentEmailMessage.where(document_id: doc_ids).joins(:email_message)
        .pluck(:document_id, "email_messages.contact_id")
        .each do |doc_id, contact_id|
          next if contact_id.nil?

          map[doc_id] ||= contact_id
        end
      map
    end

    # Persist the top positive reason + weight onto the candidate's data so the
    # card can print its kicker. A no-op without attention rows or a contact; when
    # the row has no positive reason (all structural / negative), stamp the weight
    # but drop any stale `why` so a card never prints a reason it no longer has.
    def stamp_attention(candidate)
      return if @weights_missing

      contact_id = contact_id_for(candidate[:subject])
      return if contact_id.nil?

      data = (candidate[:data] || {}).dup
      if (row = @weights_by_contact[contact_id])
        data["weight"] = row.weight.round(2)
        why = row.reason_values.find(&:positive?)
        why ? data["why"] = why.to_h : data.delete("why")
      else
        data.delete("weight")
        data.delete("why")
      end
      candidate[:data] = data
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
