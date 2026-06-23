# frozen_string_literal: true

module Campbooks
  class PageHeader < Campbooks::Base
    # @param title [String] required heading text
    # @param subtitle [String, nil] optional smaller text below title
    # @param spacing [Symbol] :md (mb-6), :lg (mb-8)
    def initialize(title:, subtitle: nil, spacing: :md, **attrs)
      @title = title
      @subtitle = subtitle
      @spacing = spacing
      @attrs = attrs
    end

    def with_actions(&block)
      @actions = block
    end

    def view_template(&content)
      # Execute the content block first so slot methods (e.g. `with_actions`)
      # register before we render. Capture any *direct* output so block-as-actions
      # usage (no `with_actions` call) still lands in the actions slot.
      captured = content ? capture(&content) : ""

      div(class: class_names("flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between", SPACING_CLASSES[@spacing]), **@attrs) do
        div(class: "min-w-0") do
          h1(class: "text-xl sm:text-2xl font-bold tracking-tight text-foreground") { @title }
          if @subtitle
            p(class: "mt-1 text-sm text-muted-foreground") { @subtitle }
          end
        end

        if @actions
          div(class: "flex items-center flex-wrap gap-3") { __yield_content__(&@actions) }
        elsif captured.present?
          div(class: "flex items-center flex-wrap gap-3") { raw(safe(captured)) }
        end
      end
    end

    private

    SPACING_CLASSES = {
      md: "mb-6",
      lg: "mb-8"
    }.freeze
  end
end
