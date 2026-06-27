# frozen_string_literal: true

module Campbooks
  # The one rich-text editor used everywhere we author HTML: the reply drawer,
  # the new-message page, the signature editor, and the document writing tool
  # (the `:document` variant unlocks tables, font family, highlight, and
  # super/subscript). It renders the wrapper + hidden input + toolbar + popovers;
  # all behaviour lives in the `tiptap-editor` Stimulus controller. Keep the
  # markup here so the toolbar is never duplicated across surfaces again.
  #
  # @param input_name  [String] name of the hidden <input> the editor HTML syncs into (e.g. "body").
  # @param content     [String] initial HTML.
  # @param placeholder [String] empty-state hint.
  # @param variant     [Symbol] :full (compose) shows the whole toolbar; :compact (signature) trims block tools.
  # @param images      [Boolean] show the image button (URL insert always works; upload needs upload_url).
  # @param upload_url  [String, nil] endpoint that accepts an "image" file and returns { url: }.
  # @param min_height  [String, nil] CSS min-height for the editing area (default 120px via CSS).
  # @param editor_class[String, nil] extra classes for the scrollable editing area (e.g. "flex-1").
  # @param wrapper_class[String, nil] extra classes for the bordered wrapper.
  # @param toolbar     [Boolean] render the toolbar.
  class RichTextEditor < Campbooks::Base
    ICONS = {
      bold:        '<path d="M14 12a4 4 0 0 0 0-8H6v8"/><path d="M15 20a4 4 0 0 0 0-8H6v8Z"/>',
      italic:      '<line x1="19" y1="4" x2="10" y2="4"/><line x1="14" y1="20" x2="5" y2="20"/><line x1="15" y1="4" x2="9" y2="20"/>',
      underline:   '<path d="M6 4v6a6 6 0 0 0 12 0V4"/><line x1="4" y1="20" x2="20" y2="20"/>',
      strike:      '<path d="M16 4H9a3 3 0 0 0-2.83 4"/><path d="M14 12a4 4 0 0 1 0 8H6"/><line x1="4" y1="12" x2="20" y2="12"/>',
      code:        '<polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>',
      align_left:   '<line x1="21" y1="6" x2="3" y2="6"/><line x1="15" y1="12" x2="3" y2="12"/><line x1="17" y1="18" x2="3" y2="18"/>',
      align_center: '<line x1="21" y1="6" x2="3" y2="6"/><line x1="17" y1="12" x2="7" y2="12"/><line x1="19" y1="18" x2="5" y2="18"/>',
      align_right:  '<line x1="21" y1="6" x2="3" y2="6"/><line x1="21" y1="12" x2="9" y2="12"/><line x1="21" y1="18" x2="7" y2="18"/>',
      bullet_list:  '<line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/>',
      ordered_list: '<line x1="10" y1="6" x2="21" y2="6"/><line x1="10" y1="12" x2="21" y2="12"/><line x1="10" y1="18" x2="21" y2="18"/><path d="M4 6h1v4"/><path d="M4 10h2"/><path d="M6 18H4c0-1 2-2 2-3s-1-1.5-2-1"/>',
      quote:        '<line x1="3" y1="5" x2="3" y2="19"/><line x1="8" y1="7" x2="21" y2="7"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="17" x2="21" y2="17"/>',
      code_block:   '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m9 10-2 2 2 2"/><path d="m15 10 2 2-2 2"/>',
      rule:         '<line x1="4" y1="12" x2="20" y2="12"/>',
      link:         '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
      image:        '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.09-3.09a2 2 0 0 0-2.82 0L6 21"/>',
      undo:         '<path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>',
      redo:         '<path d="m15 14 5-5-5-5"/><path d="M20 9H9.5a5.5 5.5 0 0 0 0 11H13"/>',
      clear:        '<path d="M4 7V4h16v3"/><line x1="5" y1="20" x2="11" y2="20"/><line x1="13" y1="4" x2="8" y2="20"/><line x1="15" y1="15" x2="20" y2="20"/><line x1="20" y1="15" x2="15" y2="20"/>',
      highlight:    '<path d="m9 11-3 3 3 3"/><path d="m8 12 6-6 4 4-6 6"/><path d="M16 4h4v4"/><line x1="8" y1="20" x2="20" y2="20"/>',
      superscript:  '<path d="m4 19 8-8"/><path d="m12 11-8 8"/><path d="M17 8h4"/><path d="m19 4 2 4"/><path d="M15 4h4"/>',
      subscript:    '<path d="m4 19 8-8"/><path d="m12 11-8 8"/><path d="M17 20h4"/><path d="m19 16 2 4"/><path d="M15 20h4"/>',
      table:        '<rect x="3" y="3" width="18" height="18" rx="2"/><line x1="3" y1="9" x2="21" y2="9"/><line x1="3" y1="15" x2="21" y2="15"/><line x1="9" y1="3" x2="9" y2="21"/><line x1="15" y1="3" x2="15" y2="21"/>',
      add_row:      '<path d="M12 5v14"/><path d="M5 12h14"/>',
      delete_row:   '<path d="M5 12h14"/>',
      add_col:      '<path d="M12 5v14"/><path d="M5 12h14"/>',
      delete_col:   '<path d="M12 5v14"/>',
      toggle_header: '<path d="M6 4h12a2 2 0 0 1 2 2v2H4V6a2 2 0 0 1 2-2z"/><path d="M4 10h16v4H4z"/><path d="M6 16H4v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2h-2"/><path d="M9 4v16"/><path d="M4 10h16"/>'
    }.freeze

    def initialize(input_name:, content: nil, placeholder: nil, variant: :full, images: true,
                   upload_url: nil, min_height: nil, editor_class: nil, wrapper_class: nil, toolbar: true,
                   tables: nil, font_family: nil, highlight: nil, superscript: nil, subscript: nil)
      @input_name = input_name
      @content = content.to_s
      @placeholder = placeholder
      @variant = variant
      @images = images
      @upload_url = upload_url
      @min_height = min_height
      @editor_class = editor_class
      @wrapper_class = wrapper_class
      @toolbar = toolbar
      @tables = tables
      @font_family = font_family
      @highlight = highlight
      @superscript = superscript
      @subscript = subscript
    end

    FEATURE_VARIANTS = {
      tables:      { document: true,  full: false, compact: false }.freeze,
      font_family: { document: true,  full: false, compact: false }.freeze,
      highlight:   { document: true,  full: false, compact: false }.freeze,
      superscript: { document: true,  full: false, compact: false }.freeze,
      subscript:   { document: true,  full: false, compact: false }.freeze
    }.freeze

    def view_template
      div(
        class: class_names(
          "rte relative border border-gray-200 rounded-lg bg-card overflow-hidden",
          "focus-within:border-accent-500 focus-within:ring-1 focus-within:ring-accent-500",
          @wrapper_class
        ),
        data: {
          controller: "tiptap-editor",
          tiptap_editor_content_value: @content,
          tiptap_editor_placeholder_value: @placeholder.to_s,
          tiptap_editor_toolbar_value: @toolbar.to_s,
          tiptap_editor_heading_value: advanced?.to_s,
          tiptap_editor_code_block_value: advanced?.to_s,
          tiptap_editor_blockquote_value: advanced?.to_s,
          tiptap_editor_upload_url_value: @upload_url.to_s,
          tiptap_editor_tables_value: feature_enabled?(:tables).to_s,
          tiptap_editor_font_family_value: feature_enabled?(:font_family).to_s,
          tiptap_editor_highlight_value: feature_enabled?(:highlight).to_s,
          tiptap_editor_super_script_value: feature_enabled?(:superscript).to_s,
          tiptap_editor_sub_script_value: feature_enabled?(:subscript).to_s
        }
      ) do
        input(type: "hidden", name: @input_name, value: @content, data: { tiptap_editor_target: "input" })
        render_toolbar if @toolbar
        link_popover
        image_popover if @images
        file_input if @images && @upload_url.present?
        div(
          data: { tiptap_editor_target: "editor" },
          style: @min_height ? "min-height: #{@min_height}" : nil,
          class: class_names("tiptap-editor", @editor_class)
        )
      end
    end

    private

    def advanced?
      @variant == :full || @variant == :document
    end

    def feature_enabled?(name)
      override = instance_variable_get(:"@#{name}")
      return override unless override.nil?
      FEATURE_VARIANTS.dig(name, @variant) || false
    end

    def render_toolbar
      div(data: { tiptap_editor_target: "toolbar" }, class: "tiptap-toolbar") do
        if advanced?
          block_type_select
          divider
        end

        tool(:bold,      "toggleBold",      t(".bold"),      active: "bold")
        tool(:italic,    "toggleItalic",    t(".italic"),    active: "italic")
        tool(:underline, "toggleUnderline", t(".underline"), active: "underline")
        tool(:strike,    "toggleStrike",    t(".strikethrough"), active: "strike")
        tool(:code, "toggleCode", t(".inline_code"), active: "code") if advanced?
        color_control
        if feature_enabled?(:highlight)
          highlight_control
        end
        divider

        tool(:align_left,   "setAlign", t(".align_left"),   active: { textAlign: "left" },   param: "left")
        tool(:align_center, "setAlign", t(".align_center"), active: { textAlign: "center" }, param: "center")
        tool(:align_right,  "setAlign", t(".align_right"),  active: { textAlign: "right" },  param: "right")
        divider

        if feature_enabled?(:superscript) || feature_enabled?(:subscript)
          tool(:superscript, "toggleSuperscript", t(".superscript"), active: "superscript") if feature_enabled?(:superscript)
          tool(:subscript, "toggleSubscript", t(".subscript"), active: "subscript") if feature_enabled?(:subscript)
          divider
        end

        tool(:bullet_list,  "toggleBulletList",  t(".bullet_list"),  active: "bulletList")
        tool(:ordered_list, "toggleOrderedList", t(".numbered_list"), active: "orderedList")
        if advanced?
          tool(:quote,      "toggleBlockquote", t(".blockquote"),     active: "blockquote")
          tool(:code_block, "toggleCodeBlock",  t(".code_block"),     active: "codeBlock")
          tool(:rule,       "setHorizontalRule", t(".horizontal_rule"))
        end
        divider

        if feature_enabled?(:tables)
          tool(:table, "insertTable", t(".table"))
          table_edit_buttons
          divider
        end

        if feature_enabled?(:font_family)
          font_family_select
          divider
        end

        tool(:link, "openLink", t(".link"), active: "link")
        tool(:image, "openImage", t(".image")) if @images
        divider

        tool(:undo, "undo", t(".undo"))
        tool(:redo, "redo", t(".redo"))
        tool(:clear, "clearFormatting", t(".clear_formatting"))
      end
    end

    def table_edit_buttons
      div(data: { rte_table_only: true }, class: "hidden flex items-center gap-0.125") do
        tool(:add_row,    "addRowBefore",    t(".add_row_before"))
        tool(:delete_row, "deleteRow",       t(".delete_row"))
        tool(:add_col,    "addColBefore",    t(".add_column_before"))
        tool(:delete_col, "deleteCol",       t(".delete_column"))
        tool(:toggle_header, "toggleHeaderCell", t(".toggle_header"))
      end
    end

    def highlight_control
      label(class: "tiptap-color", title: t(".highlight_color"), aria_label: t(".highlight_color")) do
        span(class: "tiptap-color-glyph") { "H" }
        input(
          type: "color",
          value: "#ffff00",
          data: { tiptap_editor_target: "highlightInput", action: "input->tiptap-editor#setHighlight" }
        )
      end
    end

    def font_family_select
      select(
        data: { tiptap_editor_target: "fontFamilySelect", action: "change->tiptap-editor#setFontFamily" },
        class: "tiptap-select tiptap-select--compact",
        aria_label: t(".font_family")
      ) do
        option(value: "") { t(".default_font") }
        option(value: "Inter, sans-serif") { "Inter" }
        option(value: "Georgia, serif") { "Serif" }
        option(value: "ui-monospace, monospace") { "Monospace" }
        option(value: "Arial, sans-serif") { "Arial" }
        option(value: "Times New Roman, serif") { "Times" }
        option(value: "Courier New, monospace") { "Courier" }
      end
    end

    # A toolbar button. `active` (mark name string, or attrs hash) wires the
    # live highlight; `param` passes a Stimulus action param (alignment).
    def tool(name, action, title, active: nil, param: nil)
      data = { action: "click->tiptap-editor##{action}" }
      data[:rte_active] = active.is_a?(Hash) ? active.to_json : active if active
      data[:tiptap_editor_align_param] = param if param
      button(type: "button", title: title, aria_label: title, data: data) do
        icon(name)
      end
    end

    def block_type_select
      select(
        data: { tiptap_editor_target: "blockType", action: "change->tiptap-editor#setBlockType" },
        class: "tiptap-select",
        aria_label: t(".block_type")
      ) do
        option(value: "paragraph") { t(".paragraph") }
        option(value: "h1") { t(".heading_1") }
        option(value: "h2") { t(".heading_2") }
        option(value: "h3") { t(".heading_3") }
      end
    end

    def color_control
      label(class: "tiptap-color", title: t(".text_color"), aria_label: t(".text_color")) do
        span(class: "tiptap-color-glyph") { "A" }
        input(
          type: "color",
          value: "#1c1c1c",
          data: { tiptap_editor_target: "colorInput", action: "input->tiptap-editor#setColor" }
        )
      end
    end

    def link_popover
      div(
        data: { tiptap_editor_target: "linkPopover" },
        class: "tiptap-popover"
      ) do
        input(
          type: "url",
          placeholder: t(".link_url_placeholder"),
          data: { tiptap_editor_target: "linkInput", action: "keydown->tiptap-editor#linkKeydown" },
          class: "tiptap-popover-input"
        )
        div(class: "tiptap-popover-actions") do
          button(type: "button", class: "tiptap-popover-btn tiptap-popover-btn--ghost",
                 data: { action: "click->tiptap-editor#removeLink" }) { t(".link_remove") }
          button(type: "button", class: "tiptap-popover-btn tiptap-popover-btn--primary",
                 data: { action: "click->tiptap-editor#applyLink" }) { t(".link_apply") }
        end
      end
    end

    def image_popover
      div(
        data: { tiptap_editor_target: "imagePopover" },
        class: "tiptap-popover"
      ) do
        input(
          type: "url",
          placeholder: t(".image_url_placeholder"),
          data: { tiptap_editor_target: "imageInput", action: "keydown->tiptap-editor#imageKeydown" },
          class: "tiptap-popover-input"
        )
        div(class: "tiptap-popover-actions") do
          if @upload_url.present?
            button(type: "button", class: "tiptap-popover-btn tiptap-popover-btn--ghost",
                   data: { action: "click->tiptap-editor#pickFile" }) { t(".image_upload") }
          end
          button(type: "button", class: "tiptap-popover-btn tiptap-popover-btn--primary",
                 data: { action: "click->tiptap-editor#insertImageUrl" }) { t(".image_insert") }
        end
        span(data: { tiptap_editor_target: "imageError" }, class: "tiptap-popover-error hidden")
      end
    end

    def file_input
      input(
        type: "file",
        accept: "image/*",
        hidden: true,
        data: { tiptap_editor_target: "fileInput", action: "change->tiptap-editor#uploadFile" }
      )
    end

    def divider
      span(class: "tiptap-divider")
    end

    def icon(name)
      svg(
        class: "w-4 h-4",
        fill: "none",
        stroke: "currentColor",
        stroke_width: "2",
        stroke_linecap: "round",
        stroke_linejoin: "round",
        viewBox: "0 0 24 24"
      ) { raw(safe(ICONS.fetch(name))) }
    end
  end
end
