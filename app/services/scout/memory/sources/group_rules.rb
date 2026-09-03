# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # Streams (InboxGroupRule) as sentences: "Mail from **@github.com** is a
      # stream: **Notifications**." Plus the four seeded default buckets
      # (Notifications, Newsletters & promos, Social, Updates), which read as
      # `default` — Scout treats those categories as low-priority noise.
      class GroupRules < Base
        def entries
          rule_entries + default_bucket_entries
        end

        def remove(entry)
          rule = entry.record
          return false unless rule.is_a?(InboxGroupRule) && rule.workspace_id == workspace.id

          rule.destroy
          true
        end

        private

        def rule_entries
          rules = workspace.inbox_group_rules.order(:group_name, :rule_type, :value).to_a
          org_names = label_map(Organization, rules, "organization")
          type_names = label_map(DocumentType, rules, "document_type")
          rules.map { |rule| rule_entry(rule, org_names, type_names) }
        end

        def rule_entry(rule, org_names, type_names)
          value_label = case rule.rule_type
          when "organization"  then org_names[rule.value] || rule.value
          when "document_type" then type_names[rule.value] || rule.value
          else rule.value
          end

          build(
            id: "group:#{rule.id}",
            facet: :streams,
            sentence: sentence("scout_memory.sources.group_rules.#{rule.rule_type}",
              value: value_label.to_s, group: rule.group_name),
            origin: :taught,
            origin_detail: taught_detail(rule.created_at),
            record: rule,
            form_path: routes.settings_inbox_section_path("tag_groups"),
            actions: %i[edit remove]
          )
        end

        def default_bucket_entries
          workspace.tags.where.not(default_bucket: nil).order(:default_bucket).map do |tag|
            build(
              id: "groupdefault:#{tag.default_bucket}",
              facet: :streams,
              sentence: sentence("scout_memory.sources.group_rules.default_bucket", group: tag.name),
              origin: :default,
              origin_detail: I18n.t("scout_memory.origins.default"),
              record: nil,
              form_path: routes.settings_inbox_section_path("tag_groups"),
              actions: %i[edit]
            )
          end
        end

        def label_map(model, rules, rule_type)
          ids = rules.select { |rule| rule.rule_type == rule_type }.map(&:value).uniq
          return {} if ids.empty?

          model.where(id: ids).pluck(:id, :name).to_h.transform_keys(&:to_s)
        end
      end
    end
  end
end
