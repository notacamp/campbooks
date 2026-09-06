# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # People who Scout has learned matter (or don't) to this user — derived
      # from AttentionWeight rows. Three kinds:
      #   :taught  — the user explicitly set a verdict; confirm/remove is edit
      #   :high    — top-weighted people with no verdict; confirm = agree, remove = disagree
      #   :low     — bottom-weighted people with no verdict; confirm = agree, remove = disagree
      class Attention < Base
        TOP_N    = 8
        BOTTOM_N = 4

        def entries
          taught_entries + learned_high_entries + learned_low_entries
        end

        def confirm(entry)
          kind, person_id, label = decode(entry.id)
          return false unless person_id && kind

          person = Person.find_by(id: person_id)
          return false unless person

          teach_label = kind == "low" ? "unimportant" : "important"
          ::Attention::Teach.record(person: person, user: user, label: teach_label, source: "memory")
          true
        end

        def remove(entry)
          kind, person_id, _label = decode(entry.id)
          return false unless person_id && kind

          person = Person.find_by(id: person_id)
          return false unless person

          case kind
          when "taught"
            ::Attention::Teach.forget(person: person, user: user)
          when "high"
            ::Attention::Teach.record(person: person, user: user, label: "unimportant", source: "memory")
          when "low"
            ::Attention::Teach.record(person: person, user: user, label: "important", source: "memory")
          else
            return false
          end
          true
        end

        private

        def weights
          @weights ||= AttentionWeight
            .for_user(user)
            .where(subject_type: "Person")
            .order(weight: :desc, confidence: :desc)
            .to_a
        end

        def person_names
          @person_names ||= begin
            ids = weights.map(&:subject_id)
            Person.where(id: ids).pluck(:id, :name).to_h
          end
        end

        # { person_id => newest taught label }. Reads the verdict rows first (a
        # handful) and maps only their contacts to people — never the whole
        # workspace's contact list.
        def taught_verdicts
          @taught_verdicts ||= begin
            rows = LearningDecision
              .where(domain: ::Attention::Teach::DOMAIN, user_id: user.id)
              .where.not(contact_id: nil)
              .order(created_at: :desc)
              .pluck(:contact_id, :label)
            contact_to_person = Contact.where(id: rows.map(&:first).uniq, workspace_id: workspace.id)
                                       .where.not(person_id: nil)
                                       .pluck(:id, :person_id).to_h

            verdicts = {}
            rows.each do |contact_id, label|
              pid = contact_to_person[contact_id]
              next unless pid

              verdicts[pid] ||= label # newest row per person wins
            end
            verdicts
          end
        end

        def taught_entries
          taught_verdicts.filter_map do |person_id, label|
            name = person_name_for(person_id)
            next unless name.present?

            key = label == "important" ? "taught_important" : "taught_unimportant"
            build(
              id: "attention:#{person_id}:taught",
              facet: :people,
              sentence: sentence("scout_memory.sources.attention.#{key}", name: name),
              origin: :taught,
              origin_detail: I18n.t("scout_memory.origins.taught"),
              actions: %i[remove]
            )
          end
        end

        def taught_person_ids
          @taught_person_ids ||= taught_verdicts.keys.to_set
        end

        def learned_high_entries
          weights
            .reject { |aw| taught_person_ids.include?(aw.subject_id) }
            .select { |aw| aw.confidence >= 0.5 }
            .first(TOP_N)
            .filter_map { |aw| learned_entry_for(aw, :high) }
        end

        def learned_low_entries
          weights
            .reject { |aw| taught_person_ids.include?(aw.subject_id) }
            .select { |aw| aw.weight <= 0.1 && aw.confidence >= 0.5 && aw.evidence["sender_kind"] == "person" }
            .last(BOTTOM_N)
            .filter_map { |aw| learned_entry_for(aw, :low) }
        end

        def learned_entry_for(aw, kind)
          name = person_name_for(aw.subject_id)
          return nil unless name.present?

          # A high entry explains itself with what lifts the person; a low one with
          # what keeps them out of the way ("you archive 100% of their mail unread").
          reasons = aw.reason_values
          reason = kind == :low ? (reasons.find { |r| !r.positive? } || reasons.first) : reasons.find(&:positive?)
          return nil unless reason

          reason_text = reason.clause.sub(/\.\z/, "")

          i18n_key = kind == :low ? "learned_low" : "learned"
          build(
            id: "attention:#{aw.subject_id}:#{kind}",
            facet: :people,
            sentence: sentence("scout_memory.sources.attention.#{i18n_key}", name: name, reason: reason_text),
            origin: :learned,
            origin_detail: I18n.t("scout_memory.origins.learned_attention"),
            actions: %i[confirm remove]
          )
        end

        def person_name_for(person_id)
          name = person_names[person_id]
          return name if name.present?

          # Fallback: dominant contact email
          user.workspace.contacts.where(person_id: person_id)
            .order(email_count: :desc).pick(:email)
        end

        # id shape: "attention:<person_id>:<kind>"
        def decode(id)
          parts = id.to_s.split(":", 3)
          return nil unless parts.size == 3 && parts[0] == "attention"

          _prefix, person_id, kind = parts
          [ kind, person_id, nil ]
        end
      end
    end
  end
end
