# frozen_string_literal: true

module Campbooks
  module Files
    # Structured filter controls for the Files search bar. Renders *inside* the
    # SearchBar's <form>, so every control submits with it; the bar's Stimulus
    # controller auto-submits on change. Not a standalone form.
    #
    # Names are passed as Strings, not Symbols: Campbooks::Input/Select render a
    # Symbol value through Phlex, which dasherises underscores (:date_from →
    # name="date-from") and would no longer match the controller's permitted
    # params. Strings render verbatim.
    #
    # When the active filter narrows to exactly one document type (panel type
    # checkboxes), a "Type fields" section renders per-schema-field controls and
    # a sort section provides mobile-accessible sorting.
    class FilterPanel < Campbooks::Base
      def initialize(folder: nil, filters: nil, q: nil, document_types: [], folders: [], categories: [], **attrs)
        @folder         = folder
        @filters        = filters || Documents::Filters.new
        @q              = q.to_s.strip.presence
        @document_types = document_types
        @categories     = categories
        @folders        = folders
        @attrs          = attrs
      end

      def view_template
        div(class: class_names("space-y-3", @attrs.delete(:class)), **@attrs) do
          scalar_fields
          types_section if @document_types.any?
          type_fields_section if single_doc_type
          toggles
          sort_section
          footer
        end
      end

      private

      def scalar_fields
        div(class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-x-3 gap-y-2") do
          render(Campbooks::Select.new("category", label: t(".category"),
            options: category_options, selected: @filters.categories.first,
            include_blank: t(".any_category")))
          render(Campbooks::Select.new("review_status", label: t(".review_status"),
            options: review_status_options, selected: @filters.review_status,
            include_blank: t(".any_status")))
          render(Campbooks::Select.new("ai_status", label: t(".ai_status"),
            options: ai_status_options, selected: @filters.ai_status,
            include_blank: t(".any_processing")))
          render(Campbooks::Select.new("source", label: t(".source"),
            options: source_options, selected: @filters.sources.first,
            include_blank: t(".any_source")))
          render(Campbooks::Select.new("expense_category", label: t(".expense_category"),
            options: expense_category_options, selected: @filters.expense_categories.first,
            include_blank: t(".any_expense_category")))
          if @folder.nil? && @folders.any?
            render(Campbooks::Select.new("folder_id", label: t(".folder"),
              options: folder_options, selected: @filters.folder_id,
              include_blank: t(".any_folder")))
          end
          render(Campbooks::Input.new("entity", label: t(".entity"),
            value: @filters.entities.first, placeholder: t(".entity_placeholder"), rounded: :md))
          render(Campbooks::Input.new("number", label: t(".reference_number"),
            value: @filters.numbers.first, placeholder: t(".number_placeholder"), rounded: :md))
          render(Campbooks::Input.new("date_from", type: :date, label: t(".date_from"),
            value: date_string(@filters.date_from), rounded: :md))
          render(Campbooks::Input.new("date_to", type: :date, label: t(".date_to"),
            value: date_string(@filters.date_to), rounded: :md))
          render(Campbooks::Input.new("amount_min", type: :number, label: t(".amount_min"),
            value: format_amount(@filters.amount_min_cents), placeholder: "0.00",
            rounded: :md, step: "0.01", min: "0"))
          render(Campbooks::Input.new("amount_max", type: :number, label: t(".amount_max"),
            value: format_amount(@filters.amount_max_cents), placeholder: "0.00",
            rounded: :md, step: "0.01", min: "0"))
        end
      end

      def types_section
        selected = @filters.type_ids.map(&:to_s)
        section(t(".type")) do
          div(class: "flex flex-col gap-1 max-h-40 overflow-y-auto") do
            @document_types.each do |dt|
              label(
                class: "flex items-center gap-2 cursor-pointer",
              ) do
                input(
                  type: "checkbox",
                  name: "type[]",
                  value: dt.id,
                  checked: selected.include?(dt.id.to_s),
                  class: "w-3.5 h-3.5 rounded border-gray-300 text-accent-600 focus:ring-accent-500"
                )
                render(Campbooks::ColorDot.new(color: helpers.document_type_dot_color(dt), size: :sm))
                span(class: "text-sm text-gray-700 dark:text-gray-200 truncate") { dt.name.humanize }
              end
            end
          end
        end
      end

      # Per-schema-field filter controls — only shown when exactly one type is
      # selected via the type checkboxes. Each field renders the appropriate input
      # based on its kind: text→text input, money/number/integer→min+max pair,
      # date→from+to pair, enum→select, boolean→yes/no select.
      def type_fields_section
        sdt = single_doc_type
        schema = DocumentTypes::Schema.for(sdt)
        return unless schema.any?

        section(t(".type_fields", type: sdt.name.humanize)) do
          div(class: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-x-3 gap-y-2") do
            schema.fields.each do |field|
              render_field_control(field)
            end
          end
        end
      end

      # Mobile-accessible sorting. Always rendered so phone users can sort
      # without needing the hidden table headers. The `sort` and `dir` inputs
      # submit with the enclosing SearchBar form.
      def sort_section
        section(t(".sort")) do
          div(class: "grid grid-cols-2 gap-x-3 gap-y-2") do
            render(Campbooks::Select.new("sort",
              options: sort_key_options,
              selected: helpers.params[:sort].to_s,
              aria_label: t(".sort")))
            render(Campbooks::Select.new("dir",
              options: [
                [ t(".dir_asc"), "asc" ],
                [ t(".dir_desc"), "desc" ]
              ],
              selected: helpers.params[:dir].presence || "asc",
              aria_label: t(".dir_asc")))
          end
        end
      end

      def toggles
        div(class: "flex flex-wrap items-center gap-4") do
          render(Campbooks::Toggle.new(
            name: "starred",
            label: t(".starred"),
            checked: @filters.starred,
            value: "1"
          ))
        end
      end

      # Clears the panel filters but keeps the search text (modifiers included) —
      # the query is cleared by the bar's own × control, not this link.
      def footer
        return unless @filters.any?

        clear_params = @q ? { q: @q } : {}
        clear_href = @folder ? helpers.files_folder_path(@folder, clear_params) : helpers.files_path(clear_params)
        div(class: "pt-1 border-t border-gray-100") do
          a(
            href: clear_href,
            class: "text-xs text-accent-600 hover:text-accent-700",
            data: { turbo_frame: "_top" }
          ) { t(".clear_filters") }
        end
      end

      def section(title, &)
        div(class: "space-y-1.5") do
          span(class: "block text-[10px] font-semibold uppercase tracking-wide text-gray-400") { title }
          yield
        end
      end

      # ── Helpers ──────────────────────────────────────────────────────────────

      # Derive the single selected DocumentType from panel type_ids. Returns nil
      # unless exactly one type is checked in the panel checkboxes.
      def single_doc_type
        return nil unless @filters.type_ids.size == 1

        @document_types.find { |dt| dt.id.to_s == @filters.type_ids.first.to_s }
      end

      def render_field_control(field) # rubocop:disable Metrics/MethodLength, Metrics/CyclomaticComplexity
        key = field.key
        case field.kind
        when :text
          render(Campbooks::Input.new("f[#{key}][contains]",
            label: field.label,
            value: field_filter_value(key, "contains"),
            placeholder: field.label,
            rounded: :md))
        when :money, :number, :integer
          step = field.kind == :money ? "0.01" : "1"
          render(Campbooks::Input.new("f[#{key}][min]",
            type: :number,
            label: "#{field.label} ≥",
            value: field_filter_value(key, "min"),
            placeholder: "0",
            rounded: :md, step: step, min: "0"))
          render(Campbooks::Input.new("f[#{key}][max]",
            type: :number,
            label: "#{field.label} ≤",
            value: field_filter_value(key, "max"),
            placeholder: "0",
            rounded: :md, step: step, min: "0"))
        when :date
          render(Campbooks::Input.new("f[#{key}][from]",
            type: :date,
            label: "#{field.label} ≥",
            value: field_filter_value(key, "from"),
            rounded: :md))
          render(Campbooks::Input.new("f[#{key}][to]",
            type: :date,
            label: "#{field.label} ≤",
            value: field_filter_value(key, "to"),
            rounded: :md))
        when :enum
          render(Campbooks::Select.new("f[#{key}][eq]",
            label: field.label,
            options: enum_option_pairs(key, field),
            selected: field_filter_value(key, "eq"),
            include_blank: "—"))
        when :boolean
          render(Campbooks::Select.new("f[#{key}][eq]",
            label: field.label,
            options: [
              [ t(".boolean_yes"), "true" ],
              [ t(".boolean_no"),  "false" ]
            ],
            selected: field_filter_value(key, "eq"),
            include_blank: "—"))
        end
      end

      def sort_key_options
        opts = [
          [ t(".sort_default"), "" ],
          [ t(".sort_added"),   "added" ],
          [ t(".sort_name"),    "name" ]
        ]
        if (sdt = single_doc_type)
          DocumentTypes::Schema.for(sdt).fields.each do |field|
            opts << [ field.label, field.key ]
          end
        end
        opts
      end

      # Return the stored panel value for a given field_key + op pair.
      def field_filter_value(field_key, op)
        @filters.field_filters.dig(field_key.to_s, op.to_s)
      end

      def category_options
        @categories.map { |c| [ helpers.human_enum(DocumentType, :category, c), c ] }
      end

      def review_status_options
        Document.review_statuses.keys.map { |k| [ helpers.human_enum(Document, :review_status, k), k ] }
      end

      def ai_status_options
        %w[pending processing completed failed].map { |k| [ helpers.human_enum(Document, :ai_status, k), k ] }
      end

      def source_options
        Document.sources.keys.map { |k| [ helpers.human_enum(Document, :source, k), k ] }
      end

      def expense_category_options
        Document::EXPENSE_CATEGORIES.map { |k| [ helpers.human_enum(Document, :expense_category, k), k ] }
      end

      # Localized option pairs for a schema enum field — the two well-known enums
      # reuse their i18n labels (same as chips and the document edit form).
      def enum_option_pairs(key, field)
        case key
        when "expense_category" then expense_category_options
        when "payment_method"   then helpers.payment_method_options
        else Array(field.enum_values).map { |v| [ v.to_s.humanize, v.to_s ] }
        end
      end

      def folder_options
        @folders.map { |f| [ f.name, f.id ] }
      end

      def date_string(date)
        date&.strftime("%Y-%m-%d")
      end

      def format_amount(cents)
        return nil unless cents
        "%.2f" % (cents / 100.0)
      end
    end
  end
end
