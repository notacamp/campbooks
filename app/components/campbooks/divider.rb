# frozen_string_literal: true

module Campbooks
  class Divider < Campbooks::Base
    # @param label [String, nil] optional centered text (e.g. "or")
    def initialize(label: nil, **attrs)
      @label = label
      @attrs = attrs
    end

    def view_template
      if @label
        div(class: "relative my-5", **@attrs) do
          div(class: "absolute inset-0 flex items-center") do
            div(class: "w-full border-t border-border")
          end
          div(class: "relative flex justify-center text-sm") do
            span(class: "bg-background px-3 text-muted-foreground") { @label }
          end
        end
      else
        hr(class: "border-border", **@attrs)
      end
    end
  end
end
