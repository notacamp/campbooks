module Campbooks
  # The inline "+" affordance between (and after) workflow steps. Clicking it
  # opens the shared step-picker modal (see Campbooks::StepPicker) via the
  # `step-picker` Stimulus controller on an ancestor element.
  class WorkflowStepConnector < Campbooks::Base
    def initialize(**attrs)
      @attrs = attrs
    end

    def view_template
      div(class: "flex gap-3 group", **@attrs) do
        div(class: "flex flex-col items-center") do
          div(class: "w-0.5 flex-1 bg-border")

          div(class: "flex items-center justify-center -my-1 z-10") do
            button(
              type: "button",
              class: "w-6 h-6 rounded-full border-2 border-border bg-card text-muted-foreground hover:text-foreground hover:border-foreground/40 hover:bg-accent flex items-center justify-center transition-all cursor-pointer",
              data: { action: "click->step-picker#open" },
              title: t(".add_step"),
              aria_label: t(".add_step")
            ) do
              raw safe('<svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 5v14m-7-7h14"/></svg>')
            end
          end

          div(class: "w-0.5 flex-1 bg-border")
        end

        div(class: "flex-1") do
          div(class: "py-2 opacity-0 group-hover:opacity-100 transition-opacity") do
            div(class: "text-xs text-muted-foreground") { t(".add_step_label") }
          end
        end
      end
    end
  end
end
