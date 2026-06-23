module Campbooks
  class Toast < Campbooks::Base
    # @param title [String] the notification title
    # @param body [String, nil] optional body text
    # @param time [String, nil] time string (e.g., "2 min ago")
    # @param unread [Boolean] shows a blue dot indicator when true
    # @param attrs [Hash] additional HTML attributes on the outer element
    def initialize(title:, body: nil, time: nil, unread: false, **attrs)
      @title = title
      @body = body
      @time = time
      @unread = unread
      @attrs = attrs
    end

    # Slot for action buttons (e.g., "Mark Read")
    def with_actions(&block)
      @actions = block
    end

    def view_template(&content)
      if content
        captured = capture(&content)
      end

      div(
        class: "pointer-events-auto w-full max-w-sm overflow-hidden rounded-lg bg-card text-card-foreground shadow-lg border border-border",
        role: "status",
        **@attrs
      ) do
        div(class: "p-4") do
          div(class: "flex items-start gap-3") do
            if @unread
              span(class: "w-2 h-2 rounded-full bg-primary flex-shrink-0 mt-1.5",
                   aria_label: t(".unread_label"))
            end

            div(class: "flex-1 min-w-0") do
              p(class: "text-sm font-medium text-foreground") { @title }

              if @body
                p(class: "mt-1 text-sm text-muted-foreground") { @body }
              end

              if @time
                p(class: "mt-1 text-xs text-muted-foreground") { @time }
              end
            end

            if @actions
              div(class: "flex-shrink-0", &@actions)
            end
          end
        end
      end
    end
  end
end
