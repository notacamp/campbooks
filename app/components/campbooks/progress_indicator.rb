# frozen_string_literal: true

module Campbooks
  class ProgressIndicator < Campbooks::Base
    # @param steps [Array<Hash>] array of step hashes with keys:
    #   :label [String] step label text
    #   :status [Symbol] :completed, :current, :pending
    def initialize(steps:, **attrs)
      @steps = steps
      @attrs = attrs
    end

    def view_template
      merged_class = class_names("flex items-center justify-between w-full", @attrs.delete(:class))
      div(class: merged_class, **@attrs) do
        @steps.each_with_index do |step, index|
          render_step(step, index)
          render_connector(step) unless last_step?(index)
        end
      end
    end

    private

    def render_step(step, index)
      div(class: "flex flex-col items-center") do
        div(class: step_circle_classes(step[:status])) do
          if step[:status] == :completed
            raw(safe(%(<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path></svg>)))
          else
            span(class: "text-xs font-semibold") { (index + 1).to_s }
          end
        end

        span(class: step_label_classes(step[:status])) do
          step[:label]
        end
      end
    end

    def render_connector(step)
      div(class: connector_classes(step[:status]))
    end

    def step_circle_classes(status)
      class_names(
        "w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0",
        case status
        when :completed then "bg-accent-600 text-white"
        when :current   then "ring-2 ring-accent-600 bg-card text-accent-600"
        when :pending   then "bg-card border-2 border-gray-300 text-gray-400"
        end
      )
    end

    def step_label_classes(status)
      class_names(
        "hidden sm:block text-xs mt-1 whitespace-nowrap",
        case status
        when :completed then "text-accent-700 font-medium"
        when :current   then "text-accent-600 font-medium"
        when :pending   then "text-gray-400"
        end
      )
    end

    def connector_classes(status)
      class_names(
        "h-0.5 flex-1 mx-2 self-start mt-4",
        status == :completed ? "bg-accent-600" : "bg-gray-200"
      )
    end

    def last_step?(index)
      index == @steps.length - 1
    end
  end
end
