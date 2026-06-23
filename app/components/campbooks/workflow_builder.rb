module Campbooks
  class WorkflowBuilder < Campbooks::Base
    # @param workflow [Workflow] the workflow being edited
    def initialize(workflow:, **attrs)
      @workflow = workflow
      @attrs = attrs
    end

    def view_template(&)
      div(class: "max-w-2xl", **@attrs) do
        div(class: "space-y-4") { yield if block_given? }
      end
    end

    def trigger_card(expanded: false, **attrs, &content)
      render(Campbooks::WorkflowStepCard.new(
        step_type: :trigger,
        label: t(".trigger_label"),
        summary: trigger_summary,
        expanded: expanded,
        **attrs
      ), &content)
    end

    def step_card(step:, delete_url:, expanded: false, **attrs, &content)
      render(Campbooks::WorkflowStepCard.new(
        step_type: step.step_type.to_sym,
        label: step_label(step),
        summary: step_summary(step),
        expanded: expanded,
        delete_url: delete_url,
        **attrs
      ), &content)
    end

    private

    def trigger_summary
      config = @workflow.trigger_config.with_indifferent_access
      case config[:has_documents]
      when "yes" then t(".trigger_summary.with_documents")
      when "no" then t(".trigger_summary.without_documents")
      else t(".trigger_summary.all")
      end
    end

    def step_label(step)
      case step.step_type
      when "condition"
        config = step.config.with_indifferent_access
        field = config[:field].to_s.humanize
        operator = config[:operator].to_s.humanize
        value = config[:value].to_s
        t(".step_label.condition", field: field, operator: operator.downcase, value: value)
      when "action"
        t(".step_label.send_email")
      else
        step.step_type.humanize
      end
    end

    def step_summary(step)
      case step.action_type
      when "send_email"
        config = step.config.with_indifferent_access
        account_id = config[:email_account_id]
        if account_id.present?
          account = Current.workspace&.email_accounts&.find_by(id: account_id)
          t(".step_summary.from", name: account&.display_name || t(".step_summary.unknown"))
        else
          t(".step_summary.no_account")
        end
      end
    end
  end
end
