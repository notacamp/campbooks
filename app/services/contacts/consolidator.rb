module Contacts
  class Consolidator
    SIMILARITY_THRESHOLD = 0.8

    # Auto-merge: same or very similar person name → same Person
    def self.consolidate!(person)
      return false unless person.name.present?

      duplicate = find_duplicate_person(person)
      return false unless duplicate

      merge_people(duplicate, person)
      true
    end

    # Find contacts that might belong to an existing Person (for manual review)
    def self.potential_duplicates
      Contact.without_person
             .where.not(name: nil)
             .group(:name)
             .having("count(*) > 1")
             .pluck(:name)
             .flat_map { |name| Contact.where(name: name).to_a }
    end

    def self.find_duplicate_person(person)
      normalized = normalize_name(person.name)
      # Workspace-scoped: an unscoped search would auto-merge same-named people
      # across tenants, moving their contacts into another workspace.
      person.workspace.people
            .where.not(id: person.id)
            .where.not(name: nil)
            .select { |other| similar_name?(normalized, normalize_name(other.name)) }
            .first
    end

    def self.merge_people(primary, secondary)
      Person.transaction do
        # Move all contacts to primary
        secondary.contacts.update_all(person_id: primary.id)

        # If either person is "self", the merged result must be "self"
        merged_updates = {}
        if primary.relationship_type == "self" || secondary.relationship_type == "self"
          merged_updates[:relationship_type] = "self"
        end

        # Keep the non-vendor organization (prefer orgs from non-automated contacts)
        if primary.organization.blank? || primary.relationship_type == "vendor"
          merged_updates[:organization] = secondary.organization if secondary.organization.present?
        end

        # Keep richer context summary
        if secondary.context_summary.present? &&
           secondary.context_summary.to_s.length > (primary.context_summary.to_s.length || 0)
          merged_updates.merge!(
            context_summary: secondary.context_summary,
            communication_patterns: secondary.communication_patterns,
            raw_analysis: secondary.raw_analysis,
            analyzed_at: secondary.analyzed_at
          )
        end

        primary.update_columns(merged_updates) if merged_updates.any?

        secondary.destroy!
      end
    end

    def self.normalize_name(name)
      name.to_s.strip.downcase.gsub(/\s+/, " ")
    end

    def self.similar_name?(normalized_a, normalized_b)
      return true if normalized_a == normalized_b
      max_len = [ normalized_a.length, normalized_b.length ].max
      return false if max_len == 0
      dist = levenshtein_distance(normalized_a, normalized_b)
      similarity = 1.0 - (dist.to_f / max_len)
      similarity >= SIMILARITY_THRESHOLD
    end

    def self.levenshtein_distance(s, t)
      m = s.length
      n = t.length
      return m if n == 0
      return n if m == 0

      d = (0..n).to_a
      s.chars.each_with_index do |sc, i|
        prev = d.dup
        d[0] = i + 1
        t.chars.each_with_index do |tc, j|
          cost = sc == tc ? 0 : 1
          d[j + 1] = [ d[j] + 1, prev[j + 1] + 1, prev[j] + cost ].min
        end
      end
      d[n]
    end

    private_class_method :normalize_name, :similar_name?, :levenshtein_distance
  end
end
