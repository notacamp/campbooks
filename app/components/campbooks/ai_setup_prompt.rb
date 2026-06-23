# frozen_string_literal: true

module Campbooks
  # Shown wherever an AI-dependent feature can't run because the workspace has no
  # provider for the capability it needs. It reads as Scout asking to be switched
  # on (Ember avatar = Scout) and hands the user one clear way to do it.
  #
  #   render Campbooks::AiSetupPrompt.new(capability: :text, variant: :panel)
  #
  # capability — which provider is missing, drives the copy + CTA target:
  #   :text       chat / replies / triage      → opens the setup modal
  #   :documents  PDF & image vision analysis  → opens the setup modal
  #   :embeddings semantic search              → links to Settings → AI
  # variant — where it sits:
  #   :panel   centered empty-state for a whole pane (Scout chat, compose, docs)
  #   :banner  slim strip above a section
  #   :inline  one compact line next to a disabled control
  class AiSetupPrompt < Campbooks::Base
    CAPABILITIES = %i[text documents embeddings].freeze
    VARIANTS = %i[panel banner inline].freeze

    # Capabilities whose CTA opens the setup modal rather than linking to settings.
    MODAL_CAPABILITIES = %i[text documents].freeze

    def initialize(capability: :text, variant: :panel, return_to: nil, **attrs)
      @capability = CAPABILITIES.include?(capability&.to_sym) ? capability.to_sym : :text
      @variant = VARIANTS.include?(variant&.to_sym) ? variant.to_sym : :panel
      @return_to = return_to
      @attrs = attrs
    end

    def view_template
      case @variant
      when :banner then banner_template
      when :inline then inline_template
      else panel_template
      end
    end

    private

    def panel_template
      div(class: class_names("px-6 py-10 sm:py-14 text-center", @attrs.delete(:class)), role: "status", **@attrs) do
        div(class: "mx-auto max-w-sm") do
          div(class: "flex justify-center") { render Campbooks::ScoutAvatar.new(size: :xl) }
          h2(class: "mt-4 text-base font-semibold tracking-tight text-foreground") { title }
          p(class: "mt-1.5 text-sm leading-relaxed text-muted-foreground") { body }
          div(class: "mt-5 flex justify-center") { render_cta(size: :md) }
        end
      end
    end

    def banner_template
      div(
        class: class_names(
          "flex flex-col gap-3 rounded-xl border border-border bg-muted px-4 py-3",
          "sm:flex-row sm:items-center sm:justify-between sm:gap-4",
          @attrs.delete(:class)
        ),
        role: "status", **@attrs
      ) do
        div(class: "flex items-start gap-3 sm:items-center") do
          render Campbooks::ScoutAvatar.new(size: :sm, class: "mt-0.5 sm:mt-0")
          div(class: "min-w-0") do
            p(class: "text-sm font-medium text-foreground") { title }
            p(class: "text-[13px] leading-relaxed text-muted-foreground") { body }
          end
        end
        div(class: "shrink-0") { render_cta(size: :sm) }
      end
    end

    def inline_template
      p(class: class_names("flex flex-wrap items-center gap-x-1.5 gap-y-1 text-sm text-muted-foreground", @attrs.delete(:class)), role: "status", **@attrs) do
        plain inline_lead
        whitespace
        render_link
      end
    end

    # === CTA ===

    # The pill button used by the panel and banner variants.
    def render_cta(size:)
      if modal_cta?
        render Campbooks::Button.new(variant: :primary, size: size, data: { setup_modal_open: modal_url }) { cta_label }
      else
        render Campbooks::Button.new(variant: :primary, size: size, href: helpers.settings_ai_path) { cta_label }
      end
    end

    # The text-link CTA used by the inline variant. A bare button (no href) still
    # triggers the modal — the setup-modal controller listens for the data attr
    # anywhere in the document.
    def render_link
      link_class = "font-medium text-foreground underline underline-offset-2 hover:text-foreground/80 cursor-pointer"
      if modal_cta?
        button(type: :button, class: link_class, data: { setup_modal_open: modal_url }) { cta_label }
      else
        a(href: helpers.settings_ai_path, class: link_class) { cta_label }
      end
    end

    def modal_cta?
      MODAL_CAPABILITIES.include?(@capability)
    end

    def modal_url
      helpers.setup_path(:ai_configuration, return_to: resolved_return_to)
    end

    def resolved_return_to
      @return_to || helpers.request&.path
    end

    # === Copy ===

    def title = t(".#{@capability}.title")
    def body = t(".#{@capability}.body")
    def cta_label = t(".#{@capability}.cta")
    def inline_lead = t(".inline.#{@capability}")
  end
end
