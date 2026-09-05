# frozen_string_literal: true

module Attention
  # Recomputes and materializes every attention weight for one user
  # (persons, then organizations as the max of their members). Idempotent;
  # prunes rows for counterparts that are no longer eligible. Mirrors
  # People::Standings.refresh! (upsert_all with explicit timestamps, prune,
  # transaction). Returns the number of rows written.
  module Refresh
    def self.call(user, now: Time.current)
      return 0 unless user&.workspace_id

      signals = Attention::Signals.new(user, now: now)
      facts   = signals.facts_by_person

      person_rows = []
      scored_persons = {}

      facts.each do |person_id, person_facts|
        result = Scorer.score(person_facts)
        scored_persons[person_id] = { result: result, facts: person_facts }
        person_rows << {
          user_id:       user.id,
          workspace_id:  user.workspace_id,
          subject_type:  "Person",
          subject_id:    person_id,
          weight:        result.weight,
          confidence:    result.confidence,
          raw_score:     result.raw,
          reasons:       result.reasons.map(&:to_h),
          evidence:      person_facts.to_h_for_evidence,
          last_activity_at: person_facts.last_activity_at,
          computed_at:   now,
          created_at:    now,
          updated_at:    now
        }
      rescue => e
        Rails.logger.warn("[Attention::Refresh] person #{person_id} failed: #{e.class}: #{e.message}")
      end

      org_rows = []
      signals.org_memberships.each do |org_id, member_ids|
        members = member_ids.filter_map { |pid| scored_persons[pid] && [ pid, scored_persons[pid] ] }
        next if members.empty?

        best_pid, best = members.max_by { |_pid, data| [ data[:result].weight, data[:result].confidence ] }

        display_name = signals.person_names[best_pid] || "Unknown"
        lead_reason  = Reason.new(key: "org_lead", params: { name: display_name })
        org_reasons  = [ lead_reason ] + best[:result].reasons.first(2)

        last_activity = members.filter_map { |_pid, d| d[:facts].last_activity_at }.max

        org_rows << {
          user_id:       user.id,
          workspace_id:  user.workspace_id,
          subject_type:  "Organization",
          subject_id:    org_id,
          weight:        best[:result].weight,
          confidence:    best[:result].confidence,
          raw_score:     best[:result].raw,
          reasons:       org_reasons.map(&:to_h),
          evidence:      { "lead_person_id" => best_pid, "members" => member_ids.size },
          last_activity_at: last_activity,
          computed_at:   now,
          created_at:    now,
          updated_at:    now
        }
      rescue => e
        Rails.logger.warn("[Attention::Refresh] org #{org_id} failed: #{e.class}: #{e.message}")
      end

      all_rows = person_rows + org_rows

      ApplicationRecord.transaction do
        if all_rows.any?
          AttentionWeight.upsert_all(
            all_rows,
            unique_by: :index_attention_weights_on_user_subject,
            record_timestamps: false,
            update_only: %i[weight confidence raw_score reasons evidence last_activity_at computed_at updated_at]
          )
        end

        # Prune rows no longer in this run
        fresh_set = all_rows.map { |r| [ r[:subject_type], r[:subject_id] ] }
        if fresh_set.any?
          existing = AttentionWeight.for_user(user).pluck(:subject_type, :subject_id, :id)
          fresh_lookup = fresh_set.to_set
          stale_ids = existing.filter_map { |st, sid, id| id unless fresh_lookup.include?([ st, sid ]) }
          AttentionWeight.where(id: stale_ids).delete_all if stale_ids.any?
        else
          AttentionWeight.for_user(user).delete_all
        end
      end

      all_rows.size
    end
  end
end
