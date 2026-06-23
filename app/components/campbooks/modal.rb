# frozen_string_literal: true

module Campbooks
  class Modal < Campbooks::Base
    # @param open [Boolean] whether the modal is visible
    # @param size [Symbol] :sm (w-96), :md (w-full max-w-lg), :lg (w-full max-w-2xl)
    def initialize(open: false, size: :md, **attrs)
      @open = open
      @size = size
      @attrs = attrs
    end

    def with_header(&block)
      @header = block
    end

    def with_body(&block)
      @body = block
    end

    def with_footer(&block)
      @footer = block
    end

    def view_template(&block)
      return unless @open

      yield(self) if block

      div(class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm animate-in fade-in-0 duration-200") do
        div(
          class: class_names(
            "bg-card text-card-foreground rounded-xl shadow-xl border border-border flex flex-col max-h-[80vh]",
            "animate-in fade-in-0 zoom-in-95 duration-200 ease-out",
            SIZE_CLASSES[@size]
          ),
          role: "dialog",
          aria_modal: "true",
          **@attrs
        ) do
          if @header
            div(class: "px-5 py-4 border-b border-border flex items-center justify-between", &@header)
          end

          if @body
            div(class: "flex-1 overflow-y-auto p-5", &@body)
          end

          if @footer
            div(class: "px-5 py-3 border-t border-border flex justify-end gap-2", &@footer)
          end
        end
      end
    end

    private

    SIZE_CLASSES = {
      sm: "w-full max-w-sm",
      md: "w-full max-w-lg",
      lg: "w-full max-w-2xl"
    }.freeze
  end
end
