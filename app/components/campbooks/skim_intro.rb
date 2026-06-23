# frozen_string_literal: true

module Campbooks
  # First-run explainer shown over a skim stack (Campbooks::SkimStack /
  # DocSkimStack): it says what skim is *for* and how the stories-style gesture
  # model works, then offers one warm CTA to begin. Rendered as the top layer of
  # the stack; the skim controller hides it when the user starts and remembers
  # the choice via a per-user tour flag (User#dismiss_tour!) so it greets them
  # only once. A "?" control in the stack header re-opens it on demand.
  #
  # Presentational: the host stack passes already-translated copy + steps, each
  # in its own i18n scope (components.skim_stack.intro_* / doc_skim_stack.*), so
  # this component carries only layout + icons and stays reusable across both.
  class SkimIntro < Campbooks::Base
    # @param title [String]
    # @param lead [String] one-or-two-sentence value statement (the "why")
    # @param steps [Array<Hash>] gesture rows, each { icon:, label: } where icon
    #   is a key into ICONS (:swipe, :act, :approve, :undo)
    # @param cta [String] primary button label
    # @param dismiss_action [String] Stimulus action ("controller#method") fired
    #   by the CTA to close the intro (e.g. "skim-mode#dismissIntro")
    # @param hidden [Boolean] start hidden (re-openable via header) vs shown now
    # @param attrs [Hash] extra root attrs — the host passes the stack's target
    #   plus data-tour-key (the User#dismiss_tour! key)
    def initialize(title:, lead:, steps:, cta:, dismiss_action:, hidden: false, **attrs)
      @title = title
      @lead = lead
      @steps = steps
      @cta = cta
      @dismiss_action = dismiss_action
      @hidden = hidden
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      div(
        class: class_names(
          "absolute inset-0 z-30 flex-col items-center justify-center overflow-y-auto " \
          "bg-background/85 px-6 py-10 text-center backdrop-blur-md",
          @hidden ? "hidden" : "flex",
          custom
        ),
        **@attrs
      ) do
        div(class: "m-auto w-full max-w-sm") do
          badge
          h2(class: "text-2xl font-bold tracking-tight text-foreground") { @title }
          p(class: "mx-auto mt-2 max-w-xs text-sm leading-relaxed text-muted-foreground") { @lead }
          steps_list
          cta_button
        end
      end
    end

    private

    # Warm ember tile — the product's signature accent, matching the logo and
    # Scout avatar, so the first moment of skim feels of-a-piece with the app.
    def badge
      div(
        class: "mx-auto mb-5 flex h-14 w-14 items-center justify-center rounded-2xl text-white bg-ember-gradient shadow-ember"
      ) { icon(:sparkle, size: "h-7 w-7") }
    end

    def steps_list
      ul(class: "mx-auto mt-7 max-w-xs space-y-3 text-left") do
        @steps.each do |step|
          li(class: "flex items-start gap-3") do
            span(class: "flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-xl bg-foreground/[0.06] text-foreground ring-1 ring-border/60") do
              icon(step[:icon])
            end
            span(class: "pt-1 text-sm leading-snug text-foreground") { step[:label] }
          end
        end
      end
    end

    def cta_button
      button(
        type: "button",
        class: "mt-8 inline-flex w-full items-center justify-center gap-2 rounded-xl bg-ember-gradient px-5 py-3 " \
               "text-sm font-semibold text-white shadow-ember transition-transform duration-150 ease-out active:scale-[0.98] " \
               "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-400",
        data: { action: "click->#{@dismiss_action}" }
      ) { @cta }
    end

    def icon(key, size: "h-5 w-5")
      svg(
        class: size, fill: "none", stroke: "currentColor", viewBox: "0 0 24 24",
        stroke_width: "1.8", aria_hidden: "true"
      ) { raw(safe(ICONS[key])) }
    end

    ICONS = {
      sparkle: '<path stroke-linecap="round" stroke-linejoin="round" d="M9.813 15.904 9 18.75l-.813-2.846a4.5 4.5 0 0 0-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 0 0 3.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 0 0 3.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 0 0-3.09 3.09ZM18.259 8.715 18 9.75l-.259-1.035a3.375 3.375 0 0 0-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 0 0 2.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 0 0 2.456 2.456L21.75 6l-1.035.259a3.375 3.375 0 0 0-2.456 2.456Z"/>',
      swipe: '<path stroke-linecap="round" stroke-linejoin="round" d="m5.25 4.5 7.5 7.5-7.5 7.5m6-15 7.5 7.5-7.5 7.5"/>',
      act: '<path stroke-linecap="round" stroke-linejoin="round" d="m20.25 7.5-.625 10.632a2.25 2.25 0 0 1-2.247 2.118H6.622a2.25 2.25 0 0 1-2.247-2.118L3.75 7.5M10 11.25h4M3.375 7.5h17.25c.621 0 1.125-.504 1.125-1.125v-1.5c0-.621-.504-1.125-1.125-1.125H3.375c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125Z"/>',
      approve: '<path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"/>',
      undo: '<path stroke-linecap="round" stroke-linejoin="round" d="M9 15 3 9m0 0 6-6M3 9h12a6 6 0 0 1 0 12h-3"/>'
    }.freeze
  end
end
