# frozen_string_literal: true

module Campbooks
  # Bottom-center "status feedback" pill: a small frosted capsule that floats over
  # the app to report transient or ambient state, an email sync running, an action
  # just taken and how to undo it. It's the shared shape behind the live sync
  # indicator (Campbooks::SyncIndicator) and the Skim mode undo toast, so the two
  # stay visually identical.
  #
  # The component renders an optional positioning shell wrapping the pill:
  #   - :fixed    app-global, anchored to the viewport (the sync indicator)
  #   - :absolute sits inside a positioned overlay, e.g. the Skim stack
  #   - :none     no shell; the bare pill, for the caller to place (and previews)
  #
  # The pill carries an optional leading spinner or icon, a message, and either a
  # whole-pill link (href:) or a trailing action button (action:), never both, a
  # <button> can't nest inside an <a>.
  #
  # Visibility has two modes. Static callers (the sync broadcast) render the pill
  # present and let Turbo swap the whole shell in and out. Controlled callers
  # (Skim) render it hidden: and let a Stimulus controller toggle hidden/flex, set
  # the message, and reveal the action, so the data hooks arrive via pill_data /
  # message_data / action[:data] and this component stays controller-agnostic.
  class StatusFeedback < Campbooks::Base
    SHELL_BASE = "pointer-events-none inset-x-0 flex justify-center px-4"

    # On mobile the :fixed pill rides at the *top*, tucked just under the sticky
    # topbar (Campbooks::Topbar, lg:hidden, h-14 ≈ 3.5rem) so it never collides
    # with the bottom nav (Campbooks::BottomNav, lg:hidden). At lg the topbar and
    # bottom nav are gone (the rail is lateral), so it drops to the bottom corner.
    SHELL_POSITION = {
      fixed:    "fixed top-[max(4.5rem,calc(env(safe-area-inset-top)+4rem))] lg:top-auto lg:bottom-[max(1.5rem,calc(env(safe-area-inset-bottom)+1rem))] z-40",
      absolute: "absolute bottom-[max(5.5rem,calc(env(safe-area-inset-bottom)+4.5rem))] z-30"
    }.freeze

    PILL_BASE = "pointer-events-auto items-center gap-2.5 rounded-full border border-border " \
                "bg-card/95 px-4 py-2 text-sm font-medium text-foreground shadow-lg backdrop-blur"

    ACTION_CLASSES = "rounded-md px-1.5 py-0.5 text-sm font-semibold text-primary transition-colors " \
                     "hover:bg-primary/10 focus-visible:outline focus-visible:outline-2 " \
                     "focus-visible:outline-offset-1 focus-visible:outline-accent-400"

    # @param message [String, nil] pill text (static, or the initial text for controlled mode)
    # @param spinner [Boolean] show a small leading spinner
    # @param icon [String, nil] raw inner SVG markup for a leading icon (alternative to spinner)
    # @param href [String, nil] when set, the whole pill becomes a link to this path
    # @param action [Hash, nil] trailing button: { label:, href: } or { label:, data: {} }
    # @param position [Symbol] :fixed (app-global), :absolute (inside an overlay), :none (bare pill)
    # @param hidden [Boolean] render the pill hidden, for Stimulus-controlled toggling
    # @param animate [Boolean] add a fade-in entrance
    # @param id [String, nil] id for the outer element (so Turbo Streams can target it)
    # @param pill_data [Hash] data attributes on the pill element (e.g. a Stimulus target)
    # @param message_data [Hash] data attributes on the message span (e.g. a Stimulus target)
    # @param variant [Symbol, nil] :success/:error/:warning/:info — renders a subtle
    #   colored icon badge (shares Campbooks::ActionToast's palette so the pill and
    #   the action snackbar match). Alternative to spinner:/icon:.
    # @param icon_data [Hash] data attributes on the badge element (e.g. a Stimulus
    #   target, so a controller can show/hide it per success/error).
    def initialize(message: nil, spinner: false, icon: nil, variant: nil, href: nil, action: nil,
                   position: :fixed, hidden: false, animate: false, id: nil,
                   pill_data: {}, message_data: {}, icon_data: {}, **attrs)
      @message = message
      @spinner = spinner
      @icon = icon
      @variant = variant
      @href = href
      @action = action
      @position = position
      @hidden = hidden
      @animate = animate
      @id = id
      @pill_data = pill_data
      @message_data = message_data
      @icon_data = icon_data
      @attrs = attrs
    end

    def view_template
      if @position == :none
        pill(id: @id, **@attrs)
      else
        shell_class = class_names(SHELL_BASE, SHELL_POSITION.fetch(@position), @attrs.delete(:class))
        # role/aria live on the shell so announcements work whether the pill is a
        # link or a div, and survive the pill being swapped in and out beneath it.
        div(id: @id, class: shell_class, role: "status", aria_live: "polite", **@attrs) do
          pill if show_pill?
        end
      end
    end

    private

    # Render the pill when there's something to show, or when it's a controlled
    # (hidden) instance a Stimulus controller will later reveal. An idle static
    # instance renders an empty shell that stays a stable Turbo Stream target.
    def show_pill?
      @hidden || @message.present? || @action
    end

    def pill(id: nil, **extra)
      # Hidden-mode renders with `hidden` and no display utility, so a controller
      # can swap to `flex` cleanly (see Campbooks::Base.class_names on why mixing
      # `hidden` with a display utility is unreliable).
      display = @hidden ? "hidden" : "inline-flex"
      classes = class_names(PILL_BASE, display, ("animate-fade-in" if @animate), extra.delete(:class))

      if @href
        # Ring (not outline) for the focus state: matches the app's link convention
        # and, unlike a bare `outline`, survives the Tailwind-merge in class_names.
        a(id: id, href: @href, aria_label: @message, data: @pill_data,
          class: class_names(classes, "transition-colors hover:border-foreground/20 " \
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent-400"),
          **extra) { pill_contents }
      else
        div(id: id, class: classes, data: @pill_data, **extra) { pill_contents }
      end
    end

    def pill_contents
      if @spinner
        render Campbooks::Spinner.new(size: :sm)
      elsif @variant
        variant_badge
      elsif @icon
        svg(class: "h-4 w-4 flex-shrink-0", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do
          raw(safe(@icon))
        end
      end

      span(data: @message_data) { @message }

      action_button if @action
    end

    # Subtle colored icon badge — shares Campbooks::ActionToast's palette + glyphs
    # so the skim/sync pill and the action snackbar stay visually identical.
    def variant_badge
      span(
        class: class_names(
          "inline-flex h-6 w-6 flex-shrink-0 items-center justify-center rounded-full",
          Campbooks::ActionToast::BADGE_CLASSES.fetch(@variant, Campbooks::ActionToast::BADGE_CLASSES[:info])
        ),
        data: @icon_data
      ) do
        svg(class: "h-4 w-4", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24") do
          raw(safe(Campbooks::ActionToast::ICONS.fetch(@variant, Campbooks::ActionToast::ICONS[:info])))
        end
      end
    end

    def action_button
      if @action[:href]
        a(href: @action[:href], class: ACTION_CLASSES, data: @action[:data] || {}) { @action[:label] }
      else
        button(type: "button", class: ACTION_CLASSES, data: @action[:data] || {}) { @action[:label] }
      end
    end
  end
end
