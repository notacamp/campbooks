# frozen_string_literal: true

module Campbooks
  # The one-time display of a user's recovery codes: a monospace grid plus a
  # copy-all button (reuses the `clipboard` Stimulus controller).
  class RecoveryCodeGrid < Campbooks::Base
    def initialize(codes:, **attrs)
      @codes = codes
      @attrs = attrs
    end

    def view_template
      custom_class = @attrs.delete(:class)

      div(class: class_names("rounded-lg border border-border bg-muted/50 p-4", custom_class),
          data: { controller: "clipboard" }, **@attrs) do
        # Off-screen source holding every code, newline-separated, for copy-all.
        textarea(class: "sr-only", tabindex: "-1", aria_hidden: "true",
                 data: { clipboard_target: "source" }) { @codes.join("\n") }

        ul(class: "grid grid-cols-1 sm:grid-cols-2 gap-x-8 gap-y-1.5 font-mono text-sm text-foreground") do
          @codes.each do |code|
            li(class: "tracking-widest tabular-nums") { code }
          end
        end

        div(class: "mt-4 flex justify-end") do
          render(Campbooks::Button.new(variant: :outline, size: :sm, type: "button",
                                       data: { action: "clipboard#copy", clipboard_target: "button" })) { t(".copy_all") }
        end
      end
    end
  end
end
