# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # Filing rules (EmailRule) as sentences, e.g.
      #   "File anything from **@edp.pt** under **Utilities** and archive it."
      # Composed from the rule's criteria conditions + action chips. Taught by you;
      # edit opens the rules panel, remove destroys the rule.
      class EmailRules < Base
        def entries
          workspace.email_rules.includes(:tags, :mail_folder).order(:created_at).map { |rule| entry_for(rule) }
        end

        def remove(entry)
          rule = entry.record
          return false unless rule.is_a?(EmailRule) && rule.workspace_id == workspace.id

          rule.destroy
          true
        end

        private

        def entry_for(rule)
          build(
            id: "rule:#{rule.id}",
            facet: :filing,
            sentence: rule_sentence(rule),
            origin: :taught,
            origin_detail: taught_detail(rule.created_at),
            record: rule,
            form_path: routes.settings_inbox_section_path("rules"),
            actions: %i[edit remove]
          )
        end

        def rule_sentence(rule)
          criteria = rule.criteria || {}
          conditions = condition_clause(criteria)
          actions = action_phrases(rule)
          Scout::Memory::Sentence.parse("#{assemble(conditions, actions, folder: rule.mail_folder.present?)}.")
        end

        # Grammar (keeps "under X" flowing after conditions, "and archive it"
        # otherwise) — see the honesty of the mock: no invented behaviour, just
        # the rule's own conditions and actions.
        def assemble(conditions, actions, folder:)
          lead = phrase("lead")
          prefix = conditions.blank? ? lead : "#{lead} #{conditions}"

          if folder
            "#{prefix} #{join_and(actions)}"
          elsif conditions.blank?
            "#{prefix} #{phrase('connector_and')} #{join_and(actions)}"
          else
            "#{prefix}, #{join_and(actions)}"
          end
        end

        def condition_clause(criteria)
          parts = []
          parts << fill(phrase("from"), bold_join(criteria["from"])) if present?(criteria["from"])
          parts << fill(phrase("to"), bold_join(criteria["to"])) if present?(criteria["to"])
          parts << fill(phrase("subject"), bold_join(criteria["subject"], quote: true)) if present?(criteria["subject"])
          parts << fill(phrase("body"), bold_join(criteria["body"], quote: true)) if present?(criteria["body"])
          parts << fill(phrase("category"), bold_join(category_labels(criteria["category"]))) if present?(criteria["category"])
          parts << phrase("attachment") if criteria["has_attachment"] == true
          parts.join(", ")
        end

        # Folder first (renders as "under X"), then archive / mark read / tag.
        def action_phrases(rule)
          phrases = []
          phrases << fill(phrase("folder"), bold(rule.mail_folder.name)) if rule.mail_folder
          phrases << phrase("archive") if rule.archive?
          phrases << phrase("mark_read") if rule.mark_read?
          phrases << fill(phrase("tag"), bold_join(rule.tags.map { |tag| "##{tag.name}" })) if rule.tags.any?
          phrases
        end

        def phrase(key) = I18n.t("scout_memory.sources.email_rules.#{key}")

        def fill(template, value) = template.gsub("%{value}") { value }

        def bold(value) = "**#{value.to_s.gsub('**', '')}**"

        def bold_join(values, quote: false)
          Array(values).reject(&:blank?).map { |value| bold(quote ? "\"#{value}\"" : value) }.join(", ")
        end

        def present?(value) = Array(value).reject(&:blank?).any?

        def join_and(list)
          return "" if list.empty?
          return list.first if list.one?

          "#{list[0..-2].join(', ')} #{phrase('connector_and')} #{list.last}"
        end

        def category_labels(values)
          Array(values).reject(&:blank?).map do |value|
            I18n.t("tag_groups.default_names.#{value}", default: value.to_s.humanize)
          end
        end
      end
    end
  end
end
