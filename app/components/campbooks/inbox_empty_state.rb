# frozen_string_literal: true

module Campbooks
  # The inbox zero-state, centred: Scout's avatar over a short line and, where it
  # helps, a call to action. Shared by the Home and Now deck's connect/syncing/
  # disconnected states (Home::InboxState decides which one). Copy stays under
  # home.index.* (the Home page owns these strings and its request spec asserts
  # them), read here by absolute key. The Now deck renders its own "Stack cleared."
  # for the caught-up case.
  #
  # @param state [Symbol] :syncing | :caught_up | :disconnected | :none
  # @param wrapper_class [String] outer spacing (Home uses mt-16; the deck less)
  class InboxEmptyState < Campbooks::Base
    def initialize(state:, wrapper_class: "mt-16")
      @state = state
      @wrapper_class = wrapper_class
    end

    def view_template
      div(class: class_names("flex flex-col items-center text-center", @wrapper_class)) do
        render Campbooks::ScoutAvatar.new(size: :xl)

        case @state
        when :syncing
          title(k("syncing_title"))
          body(k("syncing_body"))
          cta(helpers.email_messages_path, k("syncing_cta"))
        when :disconnected
          title(k("disconnected_title"))
          body(k("disconnected_body"))
          cta(helpers.email_messages_path(inbox_settings: "accounts"), k("disconnected_cta"))
        when :none
          title(k("connect_title"))
          body(k("connect_body"))
          connect_providers
          demo_button
        else # :caught_up
          title(k("empty_title"))
          body(k("empty_body"))
        end
      end
    end

    private

    # Absolute home.index key (this component owns no strings of its own).
    def k(key)
      helpers.t("home.index.#{key}")
    end

    def title(text)
      h2(class: "mt-4 text-lg font-semibold text-foreground") { text }
    end

    def body(text)
      p(class: "mt-1 max-w-xs text-sm text-muted-foreground") { text }
    end

    def cta(href, label)
      render(Campbooks::Button.new(variant: :primary, href: href, class: "mt-5")) { label }
    end

    def connect_providers
      div(class: "mt-6 w-full max-w-sm space-y-2.5") do
        render Campbooks::ConnectProviderCard.new(provider: :google)
        render Campbooks::ConnectProviderCard.new(provider: :zoho)
        render Campbooks::ConnectProviderCard.new(provider: :microsoft) if helpers.microsoft_enabled?
      end
    end

    def demo_button
      button(
        type: "button",
        data: { action: "product-tour#open" },
        class: "mt-4 inline-flex items-center gap-2 rounded-full border border-border bg-card px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-muted"
      ) do
        svg(viewBox: "0 0 24 24", fill: "currentColor", class: "h-4 w-4", style: "color: var(--ember-solid)", aria_hidden: "true") do
          raw(safe('<path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/>'))
        end
        plain(k("connect_demo"))
      end
    end
  end
end
