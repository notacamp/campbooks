# frozen_string_literal: true

module Campbooks
  # A document review "story card": verify the AI's classification of ONE document
  # at a glance and act on it. Shows a preview (image inline; PDFs/other as an icon
  # tile — the full file is one keystroke away via "Open"), the AI's proposed type
  # and confidence, and the key extracted fields. The document-world analogue of
  # Campbooks::SkimCard — but per-document (no clustering), and the actions are
  # Approve / Reclassify / Skip plus inline Edit, Reprocess, Open, and Flag-as-junk.
  #
  # All buttons carry data-doc-skim-action hooks; the doc-skim-mode controller
  # delegates clicks and maps the keyboard shortcuts onto the same actions. The
  # edit and reclassify panels are plain hidden data-* regions the controller
  # toggles and reads from the current frame (not Stimulus targets, since there is
  # one card per frame).
  class DocSkimCard < Campbooks::Base
    ICONS = {
      approve:    '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4.5 12.75l6 6 9-13.5"/>',
      skip:       '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.5 4.5L21 12m0 0l-7.5 7.5M21 12H3"/>',
      reclassify: '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.568 3H5.25A2.25 2.25 0 003 5.25v4.318c0 .597.237 1.17.659 1.591l9.581 9.581c.699.699 1.78.872 2.607.33a18.095 18.095 0 005.223-5.223c.542-.827.369-1.908-.33-2.607L11.16 3.66A2.25 2.25 0 009.568 3z"/><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 6h.008v.008H6V6z"/>',
      edit:       '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L10.582 16.07a4.5 4.5 0 01-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 011.13-1.897l8.932-8.931zm0 0L19.5 7.125"/>',
      reprocess:  '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99"/>',
      junk:       '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 3v1.5M3 21v-6m0 0l2.77-.693a9 9 0 016.208.682l.108.054a9 9 0 006.086.71l3.114-.732a48.524 48.524 0 01-.005-10.499l-3.11.732a9 9 0 01-6.085-.711l-.108-.054a9 9 0 00-6.208-.682L3 4.5M3 15V4.5"/>',
      open:       '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.5 6H5.25A2.25 2.25 0 003 8.25v10.5A2.25 2.25 0 005.25 21h10.5A2.25 2.25 0 0018 18.75V10.5m-10.5 6L21 3m0 0h-5.25M21 3v5.25"/>',
      file:       '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z"/>',
      expand:     '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9m5.25 11.25h-4.5m4.5 0v-4.5m0 4.5L15 15"/>',
      download:   '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5M16.5 12L12 16.5m0 0L7.5 12m4.5 4.5V3"/>'
    }.freeze

    def initialize(document_id:, category:, display_title:, entity_display_name: nil,
                   reference_display: nil, document_date: nil, amount_display: nil,
                   ai_confidence_score: nil, type_label: nil, type_color: nil, type_id: nil,
                   is_image: false, is_pdf: false, filename: nil, extracted_fields: [],
                   title_value: nil, document_types: [], file_url: nil, fill: true, **attrs)
      @document_id = document_id
      @category = category
      @display_title = display_title
      @title_value = title_value
      @entity_display_name = entity_display_name
      @reference_display = reference_display
      @document_date = document_date
      @amount_display = amount_display
      @ai_confidence_score = ai_confidence_score
      @type_label = type_label
      @type_color = type_color
      @type_id = type_id
      @is_image = is_image
      @is_pdf = is_pdf
      @filename = filename
      @extracted_fields = extracted_fields || []
      @document_types = document_types || []
      @file_url = file_url
      @fill = fill
      @attrs = attrs
    end

    def view_template
      custom = @attrs.delete(:class)
      div(
        class: class_names(
          "flex w-full flex-col rounded-xl border border-border bg-card shadow-sm",
          card_size_classes,
          custom
        ),
        **@attrs
      ) do
        header
        h3(class: "mt-3 text-balance text-xl font-bold leading-tight text-foreground sm:text-2xl", data: { doc_skim_title: true }) { @display_title }
        meta_line
        div(class: "mt-4 flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto overscroll-contain") do
          preview
          fields
          reclassify_panel
        end
        actions
      end
    end

    private

    # Type badge (what the AI thinks it is) on the left; confidence on the right.
    def header
      div(class: "flex items-center justify-between gap-3") do
        type_badge
        confidence
      end
    end

    def type_badge
      span(class: "inline-flex min-w-0 items-center gap-1.5 rounded-full bg-muted px-2.5 py-1 text-xs font-semibold text-foreground") do
        span(class: "h-2.5 w-2.5 flex-shrink-0 rounded-full", style: type_dot_style)
        span(class: "truncate") { @type_label.presence || t(".unclassified") }
      end
    end

    # All Skim docs are below the review threshold, so confidence reads low —
    # render it honestly (red < 40%, amber < 70%, green above) so the user knows
    # how much to trust the proposed type.
    def confidence
      return unless @ai_confidence_score

      pct = (@ai_confidence_score.to_f * 100).round
      hue = pct < 40 ? 25 : (pct < 70 ? 70 : 150)
      div(class: "flex flex-shrink-0 items-center gap-1.5", title: t(".ai_confidence")) do
        div(class: "h-1.5 w-14 overflow-hidden rounded-full bg-muted") do
          div(class: "h-full rounded-full", style: "width: #{pct}%; background: oklch(62% 0.18 #{hue})")
        end
        span(class: "text-[11px] font-medium text-muted-foreground tabular-nums") { "#{pct}%" }
      end
    end

    def meta_line
      parts = [ @entity_display_name, @amount_display, formatted_date, @reference_display ].map(&:presence).compact
      return if parts.empty?

      div(class: "mt-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-muted-foreground") do
        parts.each_with_index do |part, i|
          span(class: "h-1 w-1 flex-shrink-0 rounded-full bg-muted-foreground/40") if i.positive?
          span(class: "truncate") { part.to_s }
        end
      end
    end

    # Cards with an inline preview grow to fill the stage (the preview is the hero, so
    # give it real height); a preview-less file-tile card stays compact.
    def card_size_classes
      return "p-5" unless @fill

      if previewable?
        "min-h-[78vh] max-h-[88vh] p-5 sm:p-6"
      else
        "min-h-[26rem] max-h-[82vh] p-6 sm:min-h-[30rem]"
      end
    end

    # Mirrors the branches in #preview: true when we render an inline image or PDF.
    def previewable?
      (@is_image && (@file_url || @document_id)) || (@is_pdf && @document_id)
    end

    # The preview is the card's hero — it grows to fill the available height so the
    # card never looks empty for a sparse document. Images and PDFs render inline so
    # you can verify the document without leaving the flow; both carry Expand (open
    # the full-screen lightbox) and Download controls. Non-previewable files fall back
    # to an icon tile with a Download link.
    def preview
      if @is_image && (@file_url || @document_id)
        image_preview
      elsif @is_pdf && @document_id
        pdf_preview
      else
        file_tile
      end
    end

    # Inline image. The whole frame is a click target that opens the lightbox; the
    # corner controls (Expand / Download) sit above it.
    def image_preview
      div(class: "group relative flex min-h-[10rem] flex-1 items-center justify-center overflow-hidden rounded-lg border border-border bg-muted/30") do
        img(
          src: @file_url || file_path,
          alt: @filename.to_s,
          loading: "lazy",
          class: "max-h-full max-w-full object-contain"
        )
        button(
          type: "button", aria_label: t(".expand"),
          class: "absolute inset-0 cursor-zoom-in",
          data: { doc_skim_action: :preview }
        )
        preview_controls
      end
    end

    # Inline PDF. The iframe is lazily loaded by the doc-skim-mode controller (it sets
    # the src only for the active frame, so a long queue doesn't load every PDF at once)
    # and fades in over the placeholder. It's a non-interactive first-page glance (like
    # the image preview): kept out of the tab order and pointer flow, with a full-cover
    # button that opens the lightbox for the real view. The browser's PDF viewer still
    # grabs keyboard focus once when it loads (nothing declarative stops it), so the
    # controller bounces focus back to the stack — otherwise the Skim shortcuts (arrows,
    # A, R, …) would go to the plugin instead of us.
    def pdf_preview
      div(class: "relative flex min-h-[12rem] flex-1 overflow-hidden rounded-lg border border-border bg-muted/30") do
        div(class: "absolute inset-0 flex flex-col items-center justify-center gap-2 text-muted-foreground") do
          svg(class: "h-10 w-10", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[:file])) }
          span(class: "max-w-full truncate px-4 text-xs") { @filename.presence || t(".document_fallback") }
        end
        iframe(
          loading: "lazy", tabindex: "-1", aria_hidden: "true",
          title: @filename.presence || t(".document_fallback"),
          class: "pointer-events-none absolute inset-0 h-full w-full bg-card opacity-0 transition-opacity duration-200",
          data: { doc_skim_preview_frame: true, src: file_path }
        )
        button(
          type: "button", aria_label: t(".expand"),
          class: "absolute inset-0 cursor-zoom-in",
          data: { doc_skim_action: :preview }
        )
        preview_controls
      end
    end

    # Non-previewable (or missing) file: icon + filename, with a Download fallback so
    # the document is still reachable.
    def file_tile
      div(class: "flex min-h-[8rem] flex-1 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border bg-muted/40 px-4 text-muted-foreground") do
        svg(class: "h-10 w-10", fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[:file])) }
        span(class: "max-w-full truncate text-xs") { @filename.presence || t(".document_fallback") }
        if @document_id
          a(
            href: download_path, download: @filename.presence,
            class: "mt-1 inline-flex items-center gap-1 text-xs font-medium text-accent-600 hover:underline"
          ) do
            icon(:download, "h-3.5 w-3.5")
            span { t(".download") }
          end
        end
      end
    end

    # Floating Expand + Download controls, shared by the image and PDF previews.
    def preview_controls
      return unless @document_id

      div(class: "absolute right-2 top-2 z-10 flex items-center gap-1") do
        button(
          type: "button", aria_label: t(".expand"),
          class: preview_control_classes,
          data: { doc_skim_action: :preview }
        ) { icon(:expand, "h-4 w-4") }
        a(
          href: download_path, download: @filename.presence, aria_label: t(".download"),
          class: preview_control_classes
        ) { icon(:download, "h-4 w-4") }
      end
    end

    def preview_control_classes
      "inline-flex h-8 w-8 items-center justify-center rounded-md bg-background/80 text-foreground shadow-sm ring-1 ring-border backdrop-blur transition-colors hover:bg-background"
    end

    def file_path
      "/documents/#{@document_id}/file"
    end

    def download_path
      "/documents/#{@document_id}/file?disposition=attachment"
    end

    # The AI-extracted data the reviewer is signing off on, as the document type's
    # full field set (same as the detail page, via SkimBuilder/ExtractedFieldSet).
    # Collapsed by default and presented as live editable inputs — expand to verify
    # and correct values, then Save — so reviewing and fixing happen in one place.
    # doc-skim-mode reads the inputs on Save (data-doc-skim-field → column,
    # data-doc-skim-meta-field → metadata) and treats the open disclosure as a panel,
    # suppressing the nav shortcuts while you type.
    def fields
      details(class: "group/fields shrink-0 border-t border-border pt-3",
              data: { doc_skim_fields: true, action: "toggle->doc-skim-mode#onFieldsToggle" }) do
        summary(class: "flex cursor-pointer list-none select-none items-center justify-between gap-2 [&::-webkit-details-marker]:hidden") do
          span(class: "text-xs font-semibold uppercase tracking-wide text-muted-foreground/70") { t(".extracted_data") }
          svg(class: "h-4 w-4 flex-shrink-0 text-muted-foreground/60 transition-transform group-open/fields:rotate-180",
              fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") do
            raw(safe('<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>'))
          end
        end

        div(class: "mt-3 space-y-2") do
          edit_input(t(".edit_name"), "title", @title_value, placeholder: @display_title.to_s)
          if editable_fields.empty?
            p(class: "text-xs text-muted-foreground/70") { t(".no_extracted_data") }
          else
            editable_fields.each { |f| field_edit_input(f) }
          end
          div(class: "flex items-center gap-2 pt-1") do
            mini_button(t("shared.actions.save"), :primary, action: "saveFields")
          end
        end
      end
    end

    # Every field is editable inline now; only structured values (line-item arrays /
    # nested hashes) stay out of the quick editor — correct those in the full editor (O).
    def editable_fields
      @extracted_fields.reject { |f| f[:value].is_a?(Array) || f[:value].is_a?(Hash) }
    end

    def edit_value(value)
      value.is_a?(String) ? value : value&.to_s
    end

    # One labelled control, wired to the right store: column fields POST as
    # document[key] (assigned to the column), metadata fields as document[metadata][key].
    def field_edit_input(field)
      store = field[:store] == :metadata ? { doc_skim_meta_field: field[:key] } : { doc_skim_field: field[:key] }
      label(class: "block") do
        span(class: "mb-0.5 block text-xs font-medium uppercase tracking-wide text-muted-foreground/70") { field[:label] }
        field_input_control(field, store)
      end
    end

    # Typed control per field kind so values stay easy to correct: a date picker for
    # dates, a number field for cent amounts, a constrained <select> for enums.
    def field_input_control(field, store)
      case field[:kind]
      when :date
        input(type: "date", value: edit_value(field[:value]), class: field_input_classes, data: store)
      when :money
        input(type: "number", inputmode: "numeric", value: edit_value(field[:value]),
              class: field_input_classes, data: store)
      when :enum_expense_category then select_control(field, expense_category_options, store)
      when :enum_payment_method   then select_control(field, helpers.payment_method_options, store)
      else
        input(type: "text", value: edit_value(field[:value]), class: field_input_classes, data: store)
      end
    end

    # Constrained <select> (blank = unset) so only valid values are submittable; the
    # controller coerces a blank choice back to nil before assigning. `options` is a
    # list of [label, value] pairs (matching the detail page's pickers).
    def select_control(field, options, store)
      current = field[:value].to_s
      select(class: field_input_classes, data: store) do
        option(value: "") { "—" }
        options.each do |label, value|
          opts = { value: value }
          opts[:selected] = "selected" if value.to_s == current
          option(**opts) { label }
        end
      end
    end

    def expense_category_options
      Document.expense_categories.keys.map { |key| [ helpers.human_enum(Document, :expense_category, key), key ] }
    end

    # Top-level display name (metadata["title"]).
    def edit_input(label, field, value, type: "text", placeholder: nil)
      label(class: "block") do
        span(class: "mb-0.5 block text-xs font-medium uppercase tracking-wide text-muted-foreground/70") { label }
        input(
          type: type, value: value, name: field, placeholder: placeholder,
          class: field_input_classes,
          data: { doc_skim_field: field }
        )
      end
    end

    def field_input_classes
      "w-full rounded-md border border-border bg-background px-2.5 py-1.5 text-sm text-foreground focus:border-accent-400 focus:outline-none focus:ring-1 focus:ring-accent-400"
    end

    # Hidden grouped <select> the controller reveals (Reclassify / C). Options are
    # the workspace's DocumentTypes, grouped by category, current type pre-selected.
    def reclassify_panel
      div(class: "hidden shrink-0 border-t border-border pt-3", data: { doc_skim_reclassify_panel: true }) do
        span(class: "mb-1 block text-xs font-medium uppercase tracking-wide text-muted-foreground/70") { t(".refile_as") }
        select(
          class: "w-full rounded-md border border-border bg-background px-2.5 py-1.5 text-sm text-foreground focus:border-accent-400 focus:outline-none focus:ring-1 focus:ring-accent-400",
          data: { doc_skim_reclassify_select: true }
        ) do
          grouped_options
        end
        div(class: "mt-2 flex items-center justify-between gap-2") do
          div(class: "flex items-center gap-2") do
            mini_button(t(".apply"), :primary, action: "applyReclassify")
            mini_button(t("shared.actions.cancel"), :ghost, action: "cancelReclassify")
          end
          a(
            href: "/inbox_settings/document_types/new", target: "_blank", rel: "noopener",
            class: "text-xs font-medium text-accent-600 hover:underline"
          ) { t(".new_type") }
        end
      end
    end

    # Every workspace DocumentType must be selectable — types created via setup, the
    # AI analyzer, or onboarding carry no `category`, so grouping ONLY by the known
    # categories silently dropped them and left the picker empty. Group the ones with
    # a recognised category, then list the rest under "Unclassified" (or as a flat list
    # when none are categorised) so nothing is ever hidden. No types at all → a disabled
    # placeholder rather than a blank box (the "New type" link sits right below).
    def grouped_options
      if @document_types.empty?
        option(value: "", disabled: true, selected: true) { t(".no_types") }
        return
      end

      by_category = @document_types.group_by do |t|
        DocumentType::CATEGORIES.include?(t.category.to_s) ? t.category.to_s : nil
      end

      DocumentType::CATEGORIES.each do |cat|
        types = by_category[cat]
        next if types.blank?

        optgroup(label: helpers.human_enum(DocumentType, :category, cat)) do
          types.each { |t| type_option(t) }
        end
      end

      uncategorised = by_category[nil]
      return if uncategorised.blank?

      if by_category.keys.compact.any?
        optgroup(label: t(".unclassified")) { uncategorised.each { |t| type_option(t) } }
      else
        uncategorised.each { |t| type_option(t) }
      end
    end

    def type_option(type)
      option(value: type.id, selected: (type.id == @type_id)) { type.name.to_s.humanize }
    end

    def actions
      div(class: "mt-auto pt-5") do
        # Primary row — the three core decisions.
        div(class: "flex flex-wrap items-center gap-2") do
          action_button(:approve, t(".approve"), "A", style: :primary)
          action_button(:skip, t(".skip"), "→", style: :secondary)
          action_button(:reclassify, t(".reclassify"), "C", style: :secondary)
        end
        # Secondary row — the lighter-weight extras.
        div(class: "mt-2 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-xs") do
          text_action(:edit, t(".edit"), "E")
          text_action(:reprocess, t(".reprocess"), "R")
          open_action
          junk_action
        end
      end
    end

    def action_button(key, label, hint, style:)
      classes = case style
      when :primary then "bg-primary text-primary-foreground hover:bg-primary/90"
      else "border border-border text-foreground hover:bg-muted"
      end
      button(
        type: "button",
        class: class_names("inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors", classes),
        data: { doc_skim_action: key }
      ) do
        icon(key, "h-4 w-4 flex-shrink-0")
        span { label }
        kbd(class: "ml-0.5 font-mono text-[10px] opacity-60") { hint }
      end
    end

    def text_action(key, label, hint)
      button(
        type: "button",
        class: "inline-flex items-center gap-1 font-medium text-muted-foreground transition-colors hover:text-foreground",
        data: { doc_skim_action: key }
      ) do
        icon(key, "h-3.5 w-3.5 flex-shrink-0")
        span { label }
        kbd(class: "font-mono text-[10px] opacity-60") { hint }
      end
    end

    # A real <a> so a click opens the full document natively; the O shortcut just
    # clicks it (data-doc-skim-open marks it for the controller to find).
    def open_action
      a(
        href: "/documents/#{@document_id}", target: "_blank", rel: "noopener",
        class: "inline-flex items-center gap-1 font-medium text-muted-foreground transition-colors hover:text-foreground",
        data: { doc_skim_open: true }
      ) do
        icon(:open, "h-3.5 w-3.5 flex-shrink-0")
        span { t(".open") }
        kbd(class: "font-mono text-[10px] opacity-60") { "O" }
      end
    end

    def junk_action
      button(
        type: "button",
        class: "ml-auto inline-flex items-center gap-1 font-medium text-muted-foreground transition-colors hover:text-red-600 dark:hover:text-red-400",
        data: { doc_skim_action: :junk }
      ) do
        icon(:junk, "h-3.5 w-3.5 flex-shrink-0")
        span { t(".junk") }
        kbd(class: "font-mono text-[10px] opacity-60") { "J" }
      end
    end

    def mini_button(label, style, action:)
      classes = case style
      when :primary then "bg-primary text-primary-foreground hover:bg-primary/90"
      else "text-muted-foreground hover:text-foreground"
      end
      button(
        type: "button",
        class: class_names("rounded-md px-2.5 py-1 text-xs font-semibold transition-colors", classes),
        data: { doc_skim_action: action }
      ) { label }
    end

    def icon(key, klass)
      svg(class: klass, fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true") { raw(safe(ICONS[key])) }
    end

    def type_dot_style
      return "background: #{@type_color}" if @type_color.present?

      "background: oklch(58% 0.19 #{Campbooks::DocSkimTheme.hue(@category)})"
    end

    def formatted_date
      @document_date ? l(@document_date, format: :full) : nil
    end
  end
end
