# frozen_string_literal: true

module Campbooks
  # The framed shell for every unauthenticated screen — sign in, the three
  # sign-up steps, pending-approval, and the two password-reset pages. Renders a
  # raised white "paper" card, centered on a warm-grey canvas lit by one faint
  # ambient Ember halo (tokens in application.css: --auth-canvas / --auth-halo /
  # --auth-card-shadow). Glass stays reserved for Scout / topbar / the docked
  # bar; the warmth here is light, never a frosted slab.
  #
  # The card holds the brand lockup, an optional "Beta" badge, a headline and a
  # supporting line, all with the staged onboarding entrance. Everything below
  # (the form, OAuth buttons, footer links) is passed as the block and keeps its
  # own `animate-stage-in` staggering.
  #
  # The host <main> supplies the canvas background and centering via
  # `auth_canvas_class` (ApplicationHelper); this component draws only the halo
  # and the card, so the layout's flash alerts still stack above it.
  #
  # @param title [String] the headline (required)
  # @param subtitle [String, nil] supporting line; may be an html_safe string
  #   (e.g. an interpolated email in <strong>) and is then rendered as-is
  # @param badge [Boolean] show the "Beta" badge (default true; auto-hidden on
  #   self-hosted builds, matching the auth pages' existing beta gating)
  class AuthCard < Campbooks::Base
    def initialize(title:, subtitle: nil, badge: true, **attrs)
      @title = title
      @subtitle = subtitle
      @badge = badge
      @attrs = attrs
    end

    def view_template(&content)
      captured = content ? capture(&content) : ""
      custom_class = @attrs.delete(:class)

      # Ambient Ember halo — painted on the canvas behind the card (the host
      # <main> is the positioned ancestor). Purely decorative → hidden from AT.
      div(
        class: "pointer-events-none absolute inset-0 -z-10",
        style: "background: var(--auth-halo)",
        aria_hidden: "true"
      )

      div(class: "relative z-10 w-full max-w-[26.5rem]") do
        article(
          class: class_names(
            "auth-card rounded-[22px] border border-border bg-card px-6 py-8 sm:px-9 sm:py-10",
            custom_class
          ),
          **@attrs
        ) do
          render_header
          raw(safe(captured)) if captured.present?
        end
      end
    end

    private

    def render_header
      div(class: "flex flex-col items-center text-center") do
        div(class: "animate-stage-in-hero") { render Campbooks::Logo.new(size: :lg) }

        if show_badge?
          div(class: "mt-3 animate-stage-in", style: "--stage-delay: .04s") do
            render(Campbooks::Badge.new(variant: :accent, size: :md, class: "uppercase tracking-wide")) do
              t("registrations.new.beta_badge")
            end
          end
        end

        h1(
          class: "mt-5 text-2xl font-semibold leading-tight tracking-[-0.02em] text-foreground text-balance animate-stage-in",
          style: "--stage-delay: .08s"
        ) { emit(@title) }

        if @subtitle.present?
          p(
            class: "mt-2 max-w-sm text-[15px] leading-relaxed text-muted-foreground text-pretty animate-stage-in",
            style: "--stage-delay: .12s"
          ) { emit(@subtitle) }
        end
      end
    end

    def show_badge?
      @badge && !helpers.self_hosted?
    end

    # i18n titles/subtitles are trusted copy; the *_html subtitle variants arrive
    # html_safe (an interpolated email wrapped in <strong>) and must pass through
    # raw, while plain strings are escaped.
    def emit(value)
      value.respond_to?(:html_safe?) && value.html_safe? ? raw(safe(value)) : plain(value)
    end
  end
end
