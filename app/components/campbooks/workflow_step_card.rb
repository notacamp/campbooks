module Campbooks
  class WorkflowStepCard < Campbooks::Base
    # Step type is signalled by the colored icon badge below; the card itself
    # uses a neutral border (no per-type colored border).
    BG_COLORS = {
      trigger: "tone-blue",
      condition: "tone-amber",
      action: "tone-green"
    }.freeze


    # @param step_type [Symbol] :trigger, :condition, :action
    # @param label [String] the main label for the step
    # @param summary [String, nil] short description of current config
    # @param expanded [Boolean] whether the config form is shown
    # @param delete_url [String, nil] URL for the delete button
    def initialize(step_type:, label:, summary: nil, expanded: false, delete_url: nil, **attrs)
      @step_type = step_type
      @label = label
      @summary = summary
      @expanded = expanded
      @delete_url = delete_url
      @attrs = attrs
    end

    def view_template(&content)
      div(class: "flex gap-3", **@attrs) do
        div(class: "flex flex-col items-center") do
          div(class: class_names("w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0", BG_COLORS[@step_type])) do
            render_icon
          end
          div(class: "w-0.5 flex-1 bg-border my-1")
        end

        div(class: "flex-1 pb-4") do
          div(
            class: "bg-card rounded-lg border border-border shadow-sm transition-shadow group",
            data: { controller: "toggle-visibility" }
          ) do
            div(class: "flex items-center justify-between p-3 cursor-pointer", data: { action: "click->toggle-visibility#toggle" }) do
              div do
                div(class: "text-xs font-medium text-muted-foreground uppercase tracking-wide") { step_type_label }
                div(class: "text-sm font-medium text-foreground mt-0.5") { @label }
                if @summary
                  div(class: "text-xs text-muted-foreground mt-0.5") { @summary }
                end
              end

              div(class: "flex items-center gap-1") do
                if @delete_url
                  div(class: "opacity-0 group-hover:opacity-100 transition-opacity") do
                    a(
                      href: @delete_url,
                      data: { turbo_method: :delete, turbo_confirm: t(".delete_confirm") },
                      class: "text-muted-foreground hover:text-destructive p-1 cursor-pointer",
                      title: t(".delete_title")
                    ) do
                      raw safe('<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>')
                    end
                  end
                end

                div(class: class_names("transition-transform", @expanded ? "rotate-180" : "")) do
                  raw safe('<svg class="w-4 h-4 text-muted-foreground" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>')
                end
              end
            end

            div(
              class: class_names("px-3 pb-3", @expanded ? "" : "hidden"),
              data: { toggle_visibility_target: "content" }
            ) do
              div(class: "border-t border-border pt-3") do
                raw(safe(capture(&content))) if content
              end
            end
          end
        end
      end
    end

    private

    def step_type_label
      t(".step_types.#{@step_type}")
    end

    def render_icon
      svg = case @step_type
      when :trigger
        '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>'
      when :condition
        '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"/></svg>'
      when :action
        '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z"/><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>'
      end
      raw safe(svg)
    end
  end
end
