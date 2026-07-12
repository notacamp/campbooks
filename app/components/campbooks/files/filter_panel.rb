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
    class FilterPanel < Campbooks::Base
      def initialize(folder: nil, filters: nil, q: nil, document_types: [], folders: [], categories: [], **attrs)
        @folder         = folder
        @filters        = filters || Documents::Filters.new
        @q              = q.to_s.strip.presence
        @document_types = document_types
        @folders        = folders
        @categories     = categories
        @attrs          = attrs
      end

      def view_template
        div(class: class_names("space-y-3", @attrs.delete(:class)), **@attrs) do
          scalar_fields
          types_section if @document_types.any?
          toggles
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
