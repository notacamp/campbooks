# frozen_string_literal: true

module Campbooks
  # Skim-as-Stories viewer: a full-screen, Instagram-stories-style triage surface.
  #
  # THEMES are "rings" (People, Alerts, Notifications, …, plus a Priority lane);
  # each ring's clusters are the "frames" the user steps through, ordered by time
  # (Today → Earlier) so a theme walks newest-first. The rings are flattened into
  # one frame sequence, but segmented progress at the top is bounded by the
  # *current ring* so the user never faces a single 62-segment bar.
  #
  # Tap-right / → keeps the current stack (marks it addressed so it won't
  # re-surface) and advances; tap-left / ← goes back; E archives (immediately,
  # with an Undo); P pins to Priority. Progressive enhancement: with no JS the
  # first frame shows.
  class SkimStack < Campbooks::Base
    # @param rings [Array<Hash>] from Emails::SkimBuilder#rings
    # @param standalone [Boolean] true on the full-page /skim visit (renders a
    #   back-to-inbox link rather than relying on the inbox overlay's close).
    # @param start_theme [Symbol, String, nil] open on this theme's first frame
    #   (deep-link from a tray ring); nil starts at the very first frame.
    # @param show_intro [Boolean] greet first-time users with the SkimIntro
    #   overlay (the controller dismisses it + records the tour as seen).
    def initialize(rings:, standalone: false, start_theme: nil, show_intro: false, **attrs)
      @rings = rings
      @standalone = standalone
      @start_theme = start_theme
      @show_intro = show_intro
      @attrs = attrs
      @frames = flatten_frames
    end

    def view_template
      custom = @attrs.delete(:class)
      div(
        class: class_names(
          "relative flex h-full w-full flex-col overflow-hidden bg-background text-foreground outline-none",
          custom
        ),
        tabindex: "-1",
        data: {
          controller: "skim-mode",
          skim_mode_index_value: start_index,
          action: "keydown->skim-mode#onKeydown click->skim-mode#onClick " \
                   "touchstart->skim-mode#onTouchStart touchend->skim-mode#onTouchEnd"
        },
        **@attrs
      ) do
        top_chrome
        stage
        footer_hint
        toast
        email_card_layer
        intro_layer
      end
    end

    private

    # Global frame index to open on (deep-link from a tray ring).
    def start_index
      return 0 if @start_theme.blank?

      @frames.index { |frame| frame[:theme].to_s == @start_theme.to_s } || 0
    end

    # One flat list of frames in ring order, each tagged with its ring's index,
    # theme, label and per-ring position/total (assigned by SkimBuilder#rings).
    # Each cluster already carries its own time bucket_label (shown on the card).
    def flatten_frames
      @rings.each_with_index.flat_map do |ring, ring_index|
        ring[:clusters].map do |cluster|
          cluster.merge(ring_index: ring_index, ring_label: ring[:label], theme: ring[:theme])
        end
      end
    end

    def top_chrome
      div(class: "relative z-20 flex-shrink-0 px-4 pt-[max(0.75rem,env(safe-area-inset-top))] sm:px-6") do
        # Segmented progress — JS rebuilds it per ring on every frame change.
        div(
          class: "flex items-center gap-1",
          data: { skim_mode_target: "segments" },
          role: "progressbar", aria_label: t(".progress_aria")
        )

        div(class: "mt-3 flex items-center justify-between gap-3") do
          div(class: "flex min-w-0 flex-1 items-center gap-2") do
            # Per-theme icon; the skim-mode controller swaps it on every frame.
            svg(
              class: "h-4 w-4 flex-shrink-0 text-foreground/70",
              fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "1.8",
              aria_hidden: "true", data: { skim_mode_target: "ringIcon" }
            )
            ring_roadmap
          end
          div(class: "flex flex-shrink-0 items-center gap-1.5") do
            help_button
            close_button
          end
        end
      end
    end

    # A horizontal "roadmap" of the theme rings: the current bucket (emphasised,
    # with its live position) followed by the buckets still approaching (faded,
    # each with its stack count). The skim-mode controller hides the buckets
    # already done, moves the emphasis as you advance, and keeps the current one
    # scrolled to the left — so you always see what's coming next, not just the
    # bucket you're in. Past buckets render hidden; JS re-evaluates on connect.
    def ring_roadmap
      start = @frames[start_index]
      current_index = start&.dig(:ring_index) || 0
      current_position = start&.dig(:position) || 1
      div(
        class: "flex min-w-0 flex-1 items-center gap-3 overflow-x-auto " \
               "[scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
        data: { skim_mode_target: "roadmap" }
      ) do
        @rings.each_with_index do |ring, i|
          roadmap_separator(i, current_index) if i.positive?
          roadmap_chip(label: ring[:label], total: ring[:clusters].size, index: i,
                       current_index: current_index, current_position: current_position)
        end
      end
    end

    # Chevron between buckets — points from the bucket you're on toward the ones
    # still approaching, so it only shows ahead of the current one (never before
    # the leftmost/current chip). JS toggles it as you advance.
    def roadmap_separator(index, current_index)
      svg(
        class: class_names(
          "h-3 w-3 flex-shrink-0 text-muted-foreground/30",
          ("hidden" unless index > current_index)
        ),
        fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "2",
        aria_hidden: "true", data: { skim_mode_target: "roadmapSep", skim_sep_index: index }
      ) { raw(safe('<path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7"/>')) }
    end

    def roadmap_chip(label:, total:, index:, current_index:, current_position:)
      current = index == current_index
      span(
        class: class_names(
          "flex flex-shrink-0 items-center gap-1.5 whitespace-nowrap text-xs transition-colors duration-200",
          current ? "font-semibold text-foreground" : "font-medium text-muted-foreground/40",
          ("hidden" if index < current_index)
        ),
        data: { skim_mode_target: "roadmapChip", skim_chip_index: index, skim_chip_total: total }
      ) do
        plain label
        span(
          class: class_names(
            "px-1.5 py-0.5 text-[11px] font-medium tabular-nums",
            current ? "rounded-full bg-foreground/10 text-muted-foreground" : "text-muted-foreground/40"
          ),
          data: { skim_roadmap_counter: true }
        ) { current ? "#{current_position} / #{total}" : total.to_s }
      end
    end

    def stage
      div(class: "relative flex flex-1 items-center justify-center overflow-hidden px-4 py-4 sm:py-6") do
        tap_zones
        frames
        done_state
      end
    end

    # Edge strips for click-to-navigate. Behind the card (z-0); the card and its
    # buttons sit at z-10. Narrow on desktop so the centred frame stays clickable;
    # on touch, swipe is the primary path (these are an enhancement).
    def tap_zones
      button(
        type: "button", tabindex: "-1", aria_label: t(".go_back"),
        class: "absolute inset-y-0 left-0 z-0 w-1/4 cursor-default sm:w-32",
        data: { action: "click->skim-mode#prev" }
      )
      button(
        type: "button", tabindex: "-1", aria_label: t(".keep_and_continue"),
        class: "absolute inset-y-0 right-0 z-0 w-1/4 cursor-default sm:w-32",
        data: { action: "click->skim-mode#next" }
      )
    end

    # All frames are stacked absolutely; JS centres the current one and slides the
    # neighbours to the sides as dimmed peeks (desktop only). No JS → the first
    # frame is centred and the rest stay hidden.
    def frames
      div(class: "relative z-10 h-full w-full") do
        @frames.each_with_index do |frame, i|
          div(
            class: class_names(
              "absolute inset-0 flex items-center justify-center px-4 sm:px-6",
              ("hidden" unless i.zero?)
            ),
            style: ("opacity:0" unless i.zero?),
            data: {
              skim_mode_target: "frame",
              skim_ring_index: frame[:ring_index],
              skim_pos: frame[:position],
              skim_total: frame[:total],
              skim_label: frame[:ring_label],
              skim_theme: frame[:theme],
              skim_hue: Campbooks::SkimTheme.hue(frame[:theme]),
              skim_icon: Campbooks::SkimTheme.icon(frame[:theme]),
              skim_ids: Array(frame[:email_ids]).join(","),
              # Scout's learned pick for this stack — the Enter key applies it.
              skim_suggested_action: frame.dig(:scout_suggestion, :action)
            }
          ) do
            div(class: "mx-auto w-full max-w-lg sm:max-w-2xl") do
              render Campbooks::SkimCard.new(
                **frame.slice(:category, :title, :count, :summary, :samples, :emails,
                              :latest_received_at, :bucket_label, :pinned, :priority_suggested,
                              :scout_suggestion, :summary_digest, :follow_up_reason),
                theme: frame[:theme],
                show_progress: false,
                fill: true,
                class: "shadow-2xl"
              )
            end
          end
        end
      end
    end

    def done_state
      div(
        class: "absolute inset-0 z-10 hidden flex-col items-center justify-center px-6 text-center",
        data: { skim_mode_target: "done" }
      ) do
        div(class: "text-3xl") { "✨" }
        p(class: "mt-3 text-lg font-semibold") { t(".all_caught_up") }
        p(class: "mt-1 max-w-xs text-sm text-muted-foreground", data: { skim_mode_target: "summary" }) do
          t(".all_caught_up_body")
        end
        done_dismiss
      end
    end

    # Transient confirmation shown after an archive, with an Undo. The shared
    # status pill, rendered hidden; the skim-mode controller reveals it, sets the
    # message, toggles the Undo button, and auto-dismisses it after a few seconds.
    def toast
      render Campbooks::StatusFeedback.new(
        position: :absolute,
        hidden: true,
        variant: :success,
        icon_data: { skim_mode_target: "toastIcon" },
        pill_data: { skim_mode_target: "toast" },
        message_data: { skim_mode_target: "toastMessage" },
        action: {
          label: t(".undo"),
          data: { skim_mode_target: "undo", action: "click->skim-mode#undoLast" }
        }
      )
    end

    # The stacked email card: clicking a row reveals this layer and loads the
    # email into its turbo-frame, so the email sits as a card on top of the stack
    # (dimmed backdrop, the stack faintly visible behind). Backdrop tap, the
    # card's Back button, or Escape closes it. Hidden until skim-mode reveals it.
    def email_card_layer
      div(
        class: "absolute inset-0 z-40 hidden items-center justify-center bg-background/80 px-4 py-4 backdrop-blur-sm sm:py-6",
        data: { skim_mode_target: "emailLayer" }
      ) do
        button(
          type: "button", tabindex: "-1", aria_label: t(".back_to_stack"),
          class: "absolute inset-0 cursor-default",
          data: { action: "click->skim-mode#closeEmail" }
        )
        div(class: "relative z-10 mx-auto w-full max-w-lg sm:max-w-2xl") do
          raw(safe(%(<turbo-frame id="skim_email_card" class="block w-full"></turbo-frame>)))
        end
      end
    end

    # First-run explainer over the stack. Rendered hidden for returning users
    # (the header "?" re-opens it); shown on top for first-timers. The skim-mode
    # controller hides it on Start and POSTs the tour key so it greets once.
    def intro_layer
      render Campbooks::SkimIntro.new(
        title: t(".intro_title"),
        lead: t(".intro_lead"),
        steps: [
          { icon: :swipe, label: t(".intro_step_move") },
          { icon: :act,   label: t(".intro_step_act") },
          { icon: :undo,  label: t(".intro_step_undo") }
        ],
        cta: t(".intro_cta"),
        dismiss_action: "skim-mode#dismissIntro",
        hidden: !@show_intro,
        data: { skim_mode_target: "intro", tour_key: "skim_intro" }
      )
    end

    # Small "?" in the header that re-opens the intro for anyone who skipped it.
    def help_button
      button(
        type: "button", aria_label: t(".intro_help"),
        class: "inline-flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-foreground/10 text-foreground transition-colors hover:bg-foreground/20 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-400",
        data: { action: "click->skim-mode#showIntro" }
      ) do
        svg(class: "h-4 w-4", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "2", aria_hidden: "true") do
          raw(safe('<path stroke-linecap="round" stroke-linejoin="round" d="M9.879 7.519c1.171-1.025 3.071-1.025 4.242 0 1.172 1.025 1.172 2.687 0 3.712-.203.179-.43.326-.67.442-.745.361-1.45.999-1.45 1.827v.75M12 17.25h.007v.008H12v-.008Z"/><path stroke-linecap="round" stroke-linejoin="round" d="M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z"/>'))
        end
      end
    end

    def done_dismiss
      classes = "mt-3 text-xs font-medium text-muted-foreground underline-offset-2 hover:underline focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-400"
      if @standalone
        a(href: "/", class: classes, data: back_data) { t(".back_to_inbox") }
      else
        button(type: "button", class: classes, data: { action: "click->skim-overlay#close" }) { t("shared.actions.done") }
      end
    end

    def footer_hint
      div(class: "relative z-20 flex-shrink-0 px-4 pb-[max(0.75rem,env(safe-area-inset-bottom))] pt-2 text-center") do
        # Touch hint (no keys); shown on coarse pointers only.
        p(class: "text-[11px] text-muted-foreground/70 sm:hidden") { t(".touch_hint") }
        # Keyboard legend for pointer/desktop.
        p(class: "hidden text-[11px] text-muted-foreground/70 sm:block") do
          plain t(".kbd_back_keep")
          kbd(class: "rounded bg-foreground/10 px-1 py-0.5 font-mono text-[10px]") { "E" }
          plain t(".kbd_archive")
          kbd(class: "rounded bg-foreground/10 px-1 py-0.5 font-mono text-[10px]") { "P" }
          plain t(".kbd_priority")
          kbd(class: "rounded bg-foreground/10 px-1 py-0.5 font-mono text-[10px]") { "Esc" }
          plain t(".kbd_close")
        end
      end
    end

    def close_button
      classes = "inline-flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-foreground/10 text-foreground transition-colors hover:bg-foreground/20 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-400"
      if @standalone
        a(href: "/", class: classes, aria_label: t(".close_aria"), data: back_data) { close_icon }
      else
        button(type: "button", class: classes, aria_label: t(".close_aria"), data: { action: "click->skim-overlay#close" }) { close_icon }
      end
    end

    # Standalone close/back links return the user to wherever they came from
    # (history.back, falling back to the href) rather than always to the inbox.
    def back_data
      { controller: "history-back", action: "click->history-back#back" }
    end

    def close_icon
      svg(class: "h-4 w-4", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "2", aria_hidden: "true") do
        raw(safe('<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>'))
      end
    end
  end
end
