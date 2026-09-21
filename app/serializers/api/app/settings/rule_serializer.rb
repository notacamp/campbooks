# frozen_string_literal: true

module Api
  module App
    module Settings
      # Email rule as shown in inbox settings.
      class RuleSerializer
        def initialize(rule)
          @rule = rule
        end

        def as_json(*)
          {
            id: @rule.id,
            name: @rule.name,
            criteria: @rule.criteria,
            archive: @rule.archive,
            mark_read: @rule.mark_read,
            enabled: @rule.enabled,
            tag_ids: @rule.tag_ids,
            mail_folder_id: @rule.mail_folder_id,
            last_run_at: last_run_at,
            created_at: @rule.created_at
          }
        end

        private

        def last_run_at
          @rule.runs.maximum(:created_at)
        rescue
          nil
        end
      end
    end
  end
end
