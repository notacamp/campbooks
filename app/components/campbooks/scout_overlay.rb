# frozen_string_literal: true

module Campbooks
  # The global Scout overlay: a <dialog> that opens over whatever the user was
  # doing (bold layout only). It merges the classic command palette and the Scout
  # conversation into one surface driven by a single input — a sentence asks
  # Scout, a verb runs a command, a name finds a record (see the Rethink §03).
  #
  # This component is only the SHELL (head input + an empty lazy turbo-frame +
  # foot). scout_overlay_controller.js loads the body (/scout/overlay) on first
  # open, switches between browse and conversation modes, moves the one input
  # between the head and the foot, and reuses the shared command engine
  # (lib/command_items.js) so the classic palette and this overlay never fork.
  #
  #   render Campbooks::ScoutOverlay.new                       # live (in the layout)
  #   render Campbooks::ScoutOverlay.new(preview: :browse)     # Lookbook: open, browse
  #   render Campbooks::ScoutOverlay.new(preview: :conversation)
  class ScoutOverlay < Campbooks::Base
    # @param preview [false, :browse, :conversation] open the dialog + seed sample
    #   body content so the design is reviewable in Lookbook.
    def initialize(preview: false)
      @preview = preview
    end

    ESC_SVG = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-3.5 w-3.5" aria-hidden="true"><path d="M6 18 18 6M6 6l12 12"/></svg>'

    def view_template
      dialog(
        **(@preview ? { open: true } : {}),
        data: { scout_overlay_target: "dialog", action: "close->scout-overlay#onClose", ask_label: t(".ask_prefix") },
        class: "scout-overlay-dialog w-full bg-transparent p-0",
        aria: { label: t(".dialog_aria_label") }
      ) do
        div(class: "flex min-h-0 w-full flex-col overflow-hidden rounded-none border-border bg-card/90 shadow-2xl backdrop-blur-xl sm:rounded-[22px] sm:border") do
          head_row
          body_frame
          foot_row
        end
      end
    end

    private

    # HEAD — the query line. The single input lives here in browse mode; the
    # controller moves it into the foot in conversation mode. The Ember spark and
    # the Esc keycap frame it (per the mock's `.q` row).
    def head_row
      div(
        data: { scout_overlay_target: "head" },
        class: "flex items-center gap-3 border-b border-border/70 px-4 py-3.5 sm:px-5"
      ) do
        span(class: "flex-shrink-0", style: "color: var(--ember-solid)") { raw(safe(spark_svg)) }

        # Browse: the input sits in this slot. Conversation: the controller moves
        # the input out and this slot shows the active thread's title instead.
        div(data: { scout_overlay_target: "headSlot" }, class: "min-w-0 flex-1") do
          input_field(head: true)
        end
        span(
          data: { scout_overlay_target: "headTitle" },
          class: "hidden min-w-0 flex-1 truncate text-[15px] font-semibold text-foreground"
        )

        button(
          type: "button",
          data: { action: "scout-overlay#close" },
          aria: { label: t(".close") },
          class: "flex-shrink-0 rounded-md border border-border px-1.5 py-0.5 font-mono text-[10.5px] leading-none text-muted-foreground transition-colors hover:text-foreground"
        ) { plain "Esc" }
      end
    end

    # BODY — the empty lazy frame. scout_overlay_controller sets its src to
    # /scout/overlay on first open (idle) and to /scout/overlay?thread_id= when a
    # Recent thread or a new question opens a conversation.
    def body_frame
      div(class: "min-h-0 flex-1 overflow-y-auto overscroll-contain", data: { scout_overlay_target: "bodyScroll" }) do
        if @preview
          preview_body
        else
          # A bare turbo-frame (no src) — the controller fills it on first open.
          raw(safe(%(<turbo-frame id="scout_overlay_body" data-scout-overlay-target="frame" data-action="turbo:frame-load->scout-overlay#bodyLoaded"></turbo-frame>)))
        end
      end
    end

    # FOOT — hidden in browse mode. In conversation mode it holds the moved input
    # ("Ask a follow-up…") on the left and, on the right, a Recent·N toggle and a
    # link to the classic Scout history page.
    def foot_row
      div(
        data: { scout_overlay_target: "foot" },
        class: "hidden items-center gap-3 border-t border-border/70 px-4 py-3 sm:px-5",
        style: "padding-bottom: calc(0.75rem + env(safe-area-inset-bottom))"
      ) do
        div(data: { scout_overlay_target: "footSlot" }, class: "min-w-0 flex-1") do
          # In conversation-preview the input is shown here instead of the head.
          input_field(head: false) if @preview == :conversation
        end

        div(class: "flex flex-shrink-0 items-center gap-3 text-[12px]") do
          button(
            type: "button",
            data: { action: "scout-overlay#toggleRecent" },
            class: "text-muted-foreground transition-colors hover:text-foreground"
          ) do
            plain t(".recent")
            span(class: "font-mono", data: { scout_overlay_target: "recentCount" }) { @preview ? " · 6" : "" }
          end
          a(
            href: helpers.scout_path,
            data: { turbo: false },
            class: "text-muted-foreground underline decoration-border underline-offset-4 transition-colors hover:text-foreground hover:decoration-foreground"
          ) { t(".open_scout") }
        end
      end
    end

    # The one input element. Rendered once; the controller moves it between head
    # and foot. `input->scout-overlay#input` drives mode + live results;
    # `keydown->scout-overlay#keydown` runs the highlighted row / asks Scout.
    def input_field(head:)
      input(
        type: "text",
        placeholder: head ? t(".placeholder") : t(".followup_placeholder"),
        role: "combobox",
        autocomplete: "off",
        spellcheck: "false",
        aria: { label: t(".input_aria_label"), expanded: "true", controls: "scout_overlay_body", autocomplete: "list" },
        data: {
          scout_overlay_target: "input",
          action: "input->scout-overlay#input",
          followup_placeholder: t(".followup_placeholder")
        },
        class: "w-full border-0 bg-transparent p-0 text-[15px] font-semibold text-foreground placeholder:font-normal placeholder:text-muted-foreground focus:outline-none focus:ring-0 sm:text-[17px]"
      )
    end

    def spark_svg
      %(<svg viewBox="0 0 24 24" fill="currentColor" class="h-5 w-5" aria-hidden="true"><path d="M12 2l1.7 5.6L19.5 9l-5.8 1.4L12 16l-1.7-5.6L4.5 9l5.8-1.4z"/></svg>)
    end

    # ── Lookbook preview bodies ───────────────────────────────────────────────

    def preview_body
      if @preview == :conversation
        preview_conversation
      else
        preview_browse
      end
    end

    def preview_browse
      div(class: "px-4 py-4 sm:px-5") do
        div(class: "flex flex-wrap gap-1.5") do
          [ "What needs me today?", "Draft a reply to Sofia", "Show unpaid invoices" ].each do |s|
            span(class: "inline-flex items-center gap-1.5 rounded-full border border-border bg-card px-3 py-1.5 text-[12px] font-medium text-foreground/80") { s }
          end
        end
        div(class: "px-1 pb-1 pt-4 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground") { t(".recent") }
        div(class: "rounded-lg px-3 py-2 text-[13px] text-foreground/80") { "Unpaid invoices this month" }
        div(class: "rounded-lg px-3 py-2 text-[13px] text-foreground/80") { "Reply to Miguel about the contract" }
      end
    end

    def preview_conversation
      div(class: "px-4 py-4 sm:px-5") do
        div(class: "flex items-center gap-2 text-[13px] font-bold") do
          render Campbooks::ScoutAvatar.new(size: :xs)
          plain "Scout"
        end
        p(class: "mt-2 text-[14px] leading-relaxed text-foreground/90") do
          plain "Two invoices are still unpaid, totalling €612.00. I can draft a reminder for the overdue one."
        end
      end
    end
  end
end
