# frozen_string_literal: true

module Scout
  module Memory
    module Sources
      # Automations (Workflow) as sentences: "When **an email arrives**, **Send
      # Email** and **Slack Message**." One per workspace workflow. Gated on
      # Features.workflows? — yields nothing when the engine is off, so the
      # Automations facet only appears when workflows are enabled. Taught by you;
      # edit opens the workflow builder, remove destroys the workflow.
      #
      # There is deliberately no teach parser for workflows — the Teacher replies
      # "I can't learn that yet" for automation phrasings (an automation is built
      # in the visual builder, not dictated in a sentence).
      class Workflows < Base
        def entries
          return [] unless Features.workflows?

          workspace.workflows.includes(:steps).order(:created_at).map { |workflow| entry_for(workflow) }
        end

        def remove(entry)
          workflow = entry.record
          return false unless workflow.is_a?(Workflow) && workflow.workspace_id == workspace.id

          workflow.destroy
          true
        end

        private

        def entry_for(workflow)
          build(
            id: "workflow:#{workflow.id}",
            facet: :automations,
            sentence: workflow_sentence(workflow),
            origin: :taught,
            origin_detail: taught_detail(workflow.created_at),
            record: workflow,
            form_path: routes.edit_workflow_path(workflow),
            actions: %i[edit remove]
          )
        end

        def workflow_sentence(workflow)
          trigger = bold(trigger_label(workflow))
          labels = workflow.steps.filter_map { |step| action_label(step) }
          labels = [ I18n.t("scout_memory.sources.workflows.no_actions") ] if labels.empty?
          actions = join_and(labels.map { |label| bold(label) })

          string = I18n.t("scout_memory.sources.workflows.sentence")
            .gsub("%{trigger}") { trigger }
            .gsub("%{actions}") { actions }
          Scout::Memory::Sentence.parse(string)
        end

        def trigger_label(workflow)
          I18n.t("scout_memory.sources.workflows.trigger.#{workflow.trigger_type}",
            default: workflow.trigger_type.to_s.humanize)
        end

        def action_label(step)
          WorkflowStep.action_labels[step.action_type] || step.action_type.to_s.humanize
        end

        def bold(value) = "**#{value.to_s.gsub('**', '')}**"

        def join_and(list)
          return "" if list.empty?
          return list.first if list.one?

          "#{list[0..-2].join(', ')} #{I18n.t('scout_memory.sources.workflows.and')} #{list.last}"
        end
      end
    end
  end
end
