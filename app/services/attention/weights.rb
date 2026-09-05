# frozen_string_literal: true

module Attention
  # Read side: one lookup object per request/job, memoized. Every consumer of
  # relevance (PR 2: Feed::Ranking, People::Priority; PR 3: Money, Time) reads
  # through here so a missing row degrades to the same neutral prior everywhere.
  class Weights
    DEFAULT_WEIGHT = Attention::Scorer::PRIOR
    STALE_AFTER    = 15.minutes

    def initialize(user)
      @user  = user
      @cache = {}
    end

    # Person | Organization -> AttentionWeight | nil (memoized per type+id)
    def for(record)
      key = [ record.class.name, record.id ]
      return @cache[key] if @cache.key?(key)

      @cache[key] = AttentionWeight
        .for_user(@user)
        .find_by(subject_type: record.class.name, subject_id: record.id)
    end

    # Float, DEFAULT_WEIGHT when no row
    def weight(record)
      self.for(record)&.weight || DEFAULT_WEIGHT
    end

    # Batch-load a set of records (or [type, id] pairs) into the cache with one query.
    def preload(records)
      pairs = records.map do |r|
        r.is_a?(Array) ? r : [ r.class.name, r.id ]
      end

      missing = pairs.reject { |type, id| @cache.key?([ type, id ]) }
      return if missing.empty?

      rows = AttentionWeight.for_user(@user).where(
        missing.map { "(subject_type = ? AND subject_id = ?)" }.join(" OR "),
        *missing.flatten
      )

      # Build a lookup from the rows
      loaded = rows.index_by { |r| [ r.subject_type, r.subject_id ] }

      missing.each do |type, id|
        @cache[[ type, id ]] = loaded[[ type, id ]]
      end
    end

    # { person_id => AttentionWeight }
    def persons(ids)
      return {} if ids.blank?

      rows = AttentionWeight.for_user(@user)
        .where(subject_type: "Person", subject_id: ids)
      rows.each { |r| @cache[[ "Person", r.subject_id ]] = r }
      rows.index_by(&:subject_id)
    end

    # { org_id => AttentionWeight }
    def organizations(ids)
      return {} if ids.blank?

      rows = AttentionWeight.for_user(@user)
        .where(subject_type: "Organization", subject_id: ids)
      rows.each { |r| @cache[[ "Organization", r.subject_id ]] = r }
      rows.index_by(&:subject_id)
    end

    # { contact_id => AttentionWeight } via contacts.person_id
    def contacts(contact_ids)
      return {} if contact_ids.blank?

      contact_person_pairs = Contact.where(id: contact_ids).where.not(person_id: nil)
        .pluck(:id, :person_id)

      person_ids = contact_person_pairs.map(&:last).uniq
      person_map = persons(person_ids)

      contact_person_pairs.to_h do |contact_id, person_id|
        [ contact_id, person_map[person_id] ]
      end
    end

    # True when no rows exist for this user
    def missing?
      !AttentionWeight.for_user(@user).exists?
    end

    # True when missing or newest computed_at is older than threshold
    def stale?(threshold: STALE_AFTER)
      return true if missing?

      !AttentionWeight.for_user(@user).where("computed_at > ?", threshold.ago).exists?
    end

    # The top N ranked rows for this user
    def top(limit = 10)
      AttentionWeight.for_user(@user).ranked.limit(limit)
    end
  end
end
