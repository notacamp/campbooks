# frozen_string_literal: true

module Campbooks
  # Document Skim-as-Stories viewer: a full-screen, Instagram-stories-style review
  # surface for the document queue. The document-world analogue of
  # Campbooks::SkimStack.
  #
  # CATEGORIES are "rings" (Accounting, Legal, Insurance, …); each ring's documents
  # are the "frames" the user steps through. The rings are flattened into one frame
  # sequence, but segmented progress at the top is bounded by the *current ring* so
  # the bar never grows unwieldy.
  #
  # A/Enter approves the AI's classification (deferred Drive push, with Undo); → /
  # tap-right skips to the next without changing anything; ← goes back; C reclassify,
  # E edit fields, R reprocess, J flag-as-junk, O open the full editor. Progressive
  # enhancement: with no JS the first frame shows.
  class DocSkimStack < Campbooks::Base
    # @param rings [Array<Hash>] from Documents::SkimBuilder#rings
    # @param standalone [Boolean] true on the full-page /documents/skim visit
    # @param start_category [String, Symbol, nil] open on this category's first frame
    # @param document_types [Array] workspace DocumentTypes for the reclassify picker
    # @param show_intro [Boolean] greet first-time users with the SkimIntro overlay
    def initialize(rings:, standalone: false, start_category: nil, document_types: [], show_intro: false, **attrs)
      @rings = rings
      @standalone = standalone
      @start_category = start_category
      @document_types = document_types || []
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
          controller: "doc-skim-mode",
          doc_skim_mode_index_value: start_index,
          action: "keydown->doc-skim-mode#onKeydown click->doc-skim-mode#onClick " \
                   "touchstart->doc-skim-mode#onTouchStart touchend->doc-skim-mode#onTouchEnd"
        },
        **@attrs
      ) do
        top_chrome
        stage
        footer_hint
        toast
        preview_layer
        intro_layer
      end
    end

    private

    def start_index
      return 0 if @start_category.blank?

      @frames.index { |frame| frame[:category].to_s == @start_category.to_s } || 0
    end

    # One flat list of frames in ring order, each tagged with its ring's index,
    # category and label, plus the per-ring position/total from SkimBuilder.
    def flatten_frames
      @rings.each_with_index.flat_map do |ring, ring_index|
        ring[:clusters].map do |cluster|
          cluster.merge(ring_index: ring_index, ring_label: ring[:label])
        end
      end
    end

    def top_chrome
      div(class: "relative z-20 flex-shrink-0 px-4 pt-[max(0.75rem,env(safe-area-inset-top))] sm:px-6") do
        div(
          class: "flex items-center gap-1",
          data: { doc_skim_mode_target: "segments" },
          role: "progressbar", aria_label: t(".progress_aria")
        )

        div(class: "mt-3 flex items-center justify-between gap-3") do
          div(class: "flex min-w-0 flex-1 items-center gap-2") do
            svg(
              class: "h-4 w-4 flex-shrink-0 text-foreground/70",
              fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "1.8",
              aria_hidden: "true", data: { doc_skim_mode_target: "ringIcon" }
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

    # A horizontal "roadmap" of the category rings: the current bucket (emphasised,
    # with its live position) followed by the categories still approaching (faded,
    # each with its document count). The doc-skim-mode controller hides the buckets
    # already done, moves the emphasis as you advance, and keeps the current one
    # scrolled to the left — so you always see what's coming next, not just the
    # category you're in. Past buckets render hidden; JS re-evaluates on connect.
    def ring_roadmap
      start = @frames[start_index]
      current_index = start&.dig(:ring_index) || 0
      current_position = start&.dig(:position) || 1
      div(
        class: "flex min-w-0 flex-1 items-center gap-3 overflow-x-auto " \
               "[scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
        data: { doc_skim_mode_target: "roadmap" }
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
        aria_hidden: "true", data: { doc_skim_mode_target: "roadmapSep", doc_skim_sep_index: index }
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
        data: { doc_skim_mode_target: "roadmapChip", doc_skim_chip_index: index, doc_skim_chip_total: total }
      ) do
        plain label
        span(
          class: class_names(
            "px-1.5 py-0.5 text-[11px] font-medium tabular-nums",
            current ? "rounded-full bg-foreground/10 text-muted-foreground" : "text-muted-foreground/40"
          ),
          data: { doc_skim_roadmap_counter: true }
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

    def tap_zones
      button(
        type: "button", tabindex: "-1", aria_label: t(".go_back"),
        class: "absolute inset-y-0 left-0 z-0 w-1/4 cursor-default sm:w-32",
        data: { action: "click->doc-skim-mode#prev" }
      )
      button(
        type: "button", tabindex: "-1", aria_label: t(".skip_and_continue"),
        class: "absolute inset-y-0 right-0 z-0 w-1/4 cursor-default sm:w-32",
        data: { action: "click->doc-skim-mode#next" }
      )
    end

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
              doc_skim_mode_target: "frame",
              doc_skim_ring_index: frame[:ring_index],
              doc_skim_pos: frame[:position],
              doc_skim_total: frame[:total],
              doc_skim_label: frame[:ring_label],
              doc_skim_category: frame[:category],
              doc_skim_hue: Campbooks::DocSkimTheme.hue(frame[:category]),
              doc_skim_icon: Campbooks::DocSkimTheme.icon(frame[:category]),
              doc_skim_id: frame[:document_id],
              doc_skim_preview_type: preview_type(frame),
              doc_skim_filename: frame[:filename]
            }
          ) do
            div(class: "mx-auto w-full max-w-lg sm:max-w-2xl") do
              render Campbooks::DocSkimCard.new(
                **frame.slice(:document_id, :category, :display_title, :entity_display_name,
                              :reference_display, :document_date, :amount_display, :ai_confidence_score,
                              :type_label, :type_color, :type_id, :is_image, :is_pdf, :filename,
                              :extracted_fields, :title_value),
                document_types: @document_types,
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
        data: { doc_skim_mode_target: "done" }
      ) do
        div(class: "text-3xl") { "✨" }
        p(class: "mt-3 text-lg font-semibold") { t(".all_reviewed") }
        p(class: "mt-1 max-w-xs text-sm text-muted-foreground", data: { doc_skim_mode_target: "summary" }) do
          t(".all_reviewed_body")
        end
        done_dismiss
      end
    end

    def toast
      render Campbooks::StatusFeedback.new(
        position: :absolute,
        hidden: true,
        variant: :success,
        icon_data: { doc_skim_mode_target: "toastIcon" },
        pill_data: { doc_skim_mode_target: "toast" },
        message_data: { doc_skim_mode_target: "toastMessage" },
        action: {
          label: t(".undo"),
          data: { doc_skim_mode_target: "undo", action: "click->doc-skim-mode#undoLast" }
        }
      )
    end

    # First-run explainer over the review queue (doc-world analogue of
    # SkimStack#intro_layer). Hidden for returning users; the header "?" re-opens it.
    def intro_layer
      render Campbooks::SkimIntro.new(
        title: t(".intro_title"),
        lead: t(".intro_lead"),
        steps: [
          { icon: :swipe,   label: t(".intro_step_move") },
          { icon: :approve, label: t(".intro_step_act") },
          { icon: :undo,    label: t(".intro_step_undo") }
        ],
        cta: t(".intro_cta"),
        dismiss_action: "doc-skim-mode#dismissIntro",
        hidden: !@show_intro,
        data: { doc_skim_mode_target: "intro", tour_key: "doc_skim_intro" }
      )
    end

    # Small "?" in the header that re-opens the intro for anyone who skipped it.
    def help_button
      button(
        type: "button", aria_label: t(".intro_help"),
        class: "inline-flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-foreground/10 text-foreground transition-colors hover:bg-foreground/20 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-400",
        data: { action: "click->doc-skim-mode#showIntro" }
      ) do
        svg(class: "h-4 w-4", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "2", aria_hidden: "true") do
          raw(safe('<path stroke-linecap="round" stroke-linejoin="round" d="M9.879 7.519c1.171-1.025 3.071-1.025 4.242 0 1.172 1.025 1.172 2.687 0 3.712-.203.179-.43.326-.67.442-.745.361-1.45.999-1.45 1.827v.75M12 17.25h.007v.008H12v-.008Z"/><path stroke-linecap="round" stroke-linejoin="round" d="M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z"/>'))
        end
      end
    end

    def done_dismiss
      classes = "mt-3 text-xs font-medium text-muted-foreground underline-offset-2 hover:underline focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-400"
      if @standalone
        a(href: "/documents", class: classes, data: back_data) { t(".back_to_documents") }
      else
        button(type: "button", class: classes, data: { action: "click->doc-skim-overlay#close" }) { t("shared.actions.done") }
      end
    end

    # Which inline preview the card renders (mirrors DocSkimCard#preview) — the
    # controller reads this off the active frame to build the right lightbox media.
    def preview_type(frame)
      if frame[:is_image] then "image"
      elsif frame[:is_pdf] then "pdf"
      else "none"
      end
    end

    # Full-screen document lightbox: the card's Expand control (or Space) reveals it
    # and the doc-skim-mode controller fills the body with a large <img>/<iframe> for
    # the current document, plus Download and Open-in-tab links. Hidden until opened;
    # Escape or the close button dismisses it. The document-world analogue of
    # SkimStack's email_card_layer.
    def preview_layer
      div(
        class: "absolute inset-0 z-40 hidden flex-col bg-background/95 backdrop-blur-sm",
        data: { doc_skim_mode_target: "previewLayer" }
      ) do
        preview_header
        div(
          class: "relative flex-1 overflow-hidden bg-muted/20",
          data: { doc_skim_mode_target: "previewBody" }
        )
      end
    end

    def preview_header
      div(class: "flex flex-shrink-0 items-center justify-between gap-3 px-4 py-3 pt-[max(0.75rem,env(safe-area-inset-top))] sm:px-6") do
        span(
          class: "min-w-0 flex-1 truncate text-sm font-semibold",
          data: { doc_skim_mode_target: "previewTitle" }
        )
        div(class: "flex flex-shrink-0 items-center gap-1.5") do
          preview_header_link("download", "previewDownload", t(".preview_download"))
          preview_header_link("open", "previewOpen", t(".preview_open"), blank: true)
          button(
            type: "button", aria_label: t(".close_preview"),
            class: preview_header_button_classes,
            data: { action: "click->doc-skim-mode#closePreview" }
          ) { preview_icon("close") }
        end
      end
    end

    def preview_header_link(icon_key, target, label, blank: false)
      attrs = {
        href: "#", aria_label: label, title: label,
        class: preview_header_button_classes,
        data: { doc_skim_mode_target: target }
      }
      attrs[:target] = "_blank" if blank
      attrs[:rel] = "noopener" if blank
      a(**attrs) { preview_icon(icon_key) }
    end

    def preview_header_button_classes
      "inline-flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-foreground/10 text-foreground transition-colors hover:bg-foreground/20 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-400"
    end

    PREVIEW_ICONS = {
      "download" => '<path stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5M16.5 12L12 16.5m0 0L7.5 12m4.5 4.5V3"/>',
      "open"     => '<path stroke-linecap="round" stroke-linejoin="round" d="M13.5 6H5.25A2.25 2.25 0 003 8.25v10.5A2.25 2.25 0 005.25 21h10.5A2.25 2.25 0 0018 18.75V10.5m-10.5 6L21 3m0 0h-5.25M21 3v5.25"/>',
      "close"    => '<path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>'
    }.freeze

    def preview_icon(key)
      svg(class: "h-4 w-4", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", stroke_width: "2", aria_hidden: "true") do
        raw(safe(PREVIEW_ICONS[key]))
      end
    end

    def footer_hint
      div(class: "relative z-20 flex-shrink-0 px-4 pb-[max(0.75rem,env(safe-area-inset-bottom))] pt-2 text-center") do
        p(class: "text-[11px] text-muted-foreground/70 sm:hidden") { t(".touch_hint") }
        p(class: "hidden flex-wrap items-center justify-center gap-x-2 gap-y-0.5 text-[11px] text-muted-foreground/70 sm:flex") do
          legend_key("←", t(".kbd_back"))
          legend_key("→", t(".kbd_skip"))
          legend_key("A", t(".kbd_approve"))
          legend_key("C", t(".kbd_reclassify"))
          legend_key("E", t(".kbd_edit"))
          legend_key("R", t(".kbd_reprocess"))
          legend_key("J", t(".kbd_junk"))
          legend_key("O", t(".kbd_open"))
          legend_key("Space", t(".kbd_preview"))
          legend_key("Esc", t(".kbd_close"))
        end
      end
    end

    def legend_key(key, label)
      span(class: "inline-flex items-center gap-1") do
        kbd(class: "rounded bg-foreground/10 px-1 py-0.5 font-mono text-[10px]") { key }
        plain label
      end
    end

    def close_button
      classes = "inline-flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full bg-foreground/10 text-foreground transition-colors hover:bg-foreground/20 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-400"
      if @standalone
        a(href: "/documents", class: classes, aria_label: t(".close_aria"), data: back_data) { close_icon }
      else
        button(type: "button", class: classes, aria_label: t(".close_aria"), data: { action: "click->doc-skim-overlay#close" }) { close_icon }
      end
    end

    # Standalone close/back links return the user to wherever they came from
    # (history.back, falling back to the href) rather than always to /documents.
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
