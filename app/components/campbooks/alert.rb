module Campbooks
  class Alert < Campbooks::Base
    # Canonical variants are success/error/warning/info. notice/alert are kept as
    # aliases (notice→green, alert→red) so legacy callers keep working.
    VARIANT_CLASSES = {
      success: "bg-green-50 text-green-800 border-green-200 dark:bg-green-500/10 dark:text-green-300 dark:border-green-500/25",
      notice:  "bg-green-50 text-green-800 border-green-200 dark:bg-green-500/10 dark:text-green-300 dark:border-green-500/25",
      error:   "bg-red-50 text-red-800 border-red-200 dark:bg-red-500/10 dark:text-red-300 dark:border-red-500/25",
      alert:   "bg-red-50 text-red-800 border-red-200 dark:bg-red-500/10 dark:text-red-300 dark:border-red-500/25",
      warning: "bg-amber-50 text-amber-800 border-amber-200 dark:bg-amber-500/10 dark:text-amber-300 dark:border-amber-500/25",
      info:    "bg-blue-50 text-blue-800 border-blue-200 dark:bg-blue-500/10 dark:text-blue-300 dark:border-blue-500/25"
    }.freeze

    ROLE = {
      success: "status",
      notice:  "status",
      error:   "alert",
      alert:   "alert",
      warning: "status",
      info:    "status"
    }.freeze

    # @param variant [Symbol] :success, :error, :warning, :info (notice/alert aliased)
    # @param message [String, nil] optional message string shortcut
    def initialize(variant: :info, message: nil, **attrs)
      @variant = variant
      @message = message
      @attrs = attrs
    end

    def view_template(&content)
      div(
        class: class_names("mb-4 rounded-md border p-4 text-sm", VARIANT_CLASSES[@variant]),
        role: ROLE[@variant],
        **@attrs
      ) do
        if @message
          p { @message }
        else
          __yield_content__(&content)
        end
      end
    end
  end
end
