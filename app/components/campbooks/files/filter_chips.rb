# frozen_string_literal: true

module Campbooks
  module Files
    # Removable chips for active panel filter values, rendered inside the
    # `files_results` Turbo Frame so they refresh together with the results.
    # Each chip is a link that removes that specific filter value by navigating
    # to the current URL with that value omitted.
    #
    # Modifier-derived constraints (from the query text) are NOT shown here —
    # they're already visible in the search bar. Only panel params appear.
    class FilterChips < Campbooks::Base
      CLOSE_PATH = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>'.freeze

      def initialize(filters:, q: nil, folder: nil, document_types: [], folders: [], single_type: nil)
        @filters        = filters
        @q              = q.to_s.strip.presence
        @folder         = folder
        @document_types = document_types
        @folders        = folders
        @single_type    = single_type
      end

      def view_template
        chips = build_chips
        return if chips.empty?

        div(class: "flex flex-wrap items-center gap-1 px-2.5 py-1.5 border-b border-gray-100") do
          chips.each do |chip|
            render_chip(chip[:label], chip[:remove_path])
          end
        end
      end

      private

      def build_chips # rubocop:disable Metrics/AbcSize,Metrics/MethodLength
        chips = []

        # Document type — one chip per selected type id.
        @filters.type_ids.each do |type_id|
          dt    = @document_types.find { |d| d.id.to_s == type_id.to_s }
          label = dt ? dt.name.humanize : type_id.to_s
          chips << { label: label, remove_path: remove_array_item(:type, type_id) }
        end

        # Category — one chip per selected category.
        @filters.categories.each do |cat|
          chips << {
            label: helpers.human_enum(DocumentType, :category, cat),
            remove_path: remove_array_item(:category, cat)
          }
        end

        # Review status.
        if @filters.review_status.present?
          chips << {
            label: helpers.human_enum(Document, :review_status, @filters.review_status),
            remove_path: remove_single(:review_status)
          }
        end

        # AI status.
        if @filters.ai_status.present?
          chips << {
            label: helpers.human_enum(Document, :ai_status, @filters.ai_status),
            remove_path: remove_single(:ai_status)
          }
        end

        # Source — one chip per selected source.
        @filters.sources.each do |src|
          chips << {
            label: helpers.human_enum(Document, :source, src),
            remove_path: remove_array_item(:source, src)
          }
        end

        # Starred toggle.
        if @filters.starred
          chips << { label: t(".starred"), remove_path: remove_single(:starred) }
        end

        # Date from.
        if @filters.date_from.present?
          chips << {
            label: "#{t(".date_from_prefix")} #{l(@filters.date_from, format: :default)}",
            remove_path: remove_single(:date_from)
          }
        end

        # Date to.
        if @filters.date_to.present?
          chips << {
            label: "#{t(".date_to_prefix")} #{l(@filters.date_to, format: :default)}",
            remove_path: remove_single(:date_to)
          }
        end

        # Amount min.
        if @filters.amount_min_cents.present?
          chips << {
            label: "≥ #{format_amount(@filters.amount_min_cents)}",
            remove_path: remove_single(:amount_min)
          }
        end

        # Amount max.
        if @filters.amount_max_cents.present?
          chips << {
            label: "≤ #{format_amount(@filters.amount_max_cents)}",
            remove_path: remove_single(:amount_max)
          }
        end

        # Entity — one chip per term.
        @filters.entities.each do |entity|
          chips << {
            label: "\"#{entity}\"",
            remove_path: remove_array_item(:entity, entity)
          }
        end

        # Reference number — one chip per term.
        @filters.numbers.each do |num|
          chips << {
            label: "\"#{num}\"",
            remove_path: remove_array_item(:number, num)
          }
        end

        # Expense category — one chip per selection.
        @filters.expense_categories.each do |ec|
          chips << {
            label: helpers.human_enum(Document, :expense_category, ec),
            remove_path: remove_array_item(:expense_category, ec)
          }
        end

        # Folder (only in all-files view — not shown when browsing a specific folder).
        if @filters.folder_id.present? && @folder.nil?
          folder_obj = @folders.find { |f| f.id.to_s == @filters.folder_id.to_s }
          chips << {
            label: folder_obj ? folder_obj.name : @filters.folder_id,
            remove_path: remove_single(:folder_id)
          }
        end

        # Per-schema-field filter chips — only when single type is active.
        chips.concat(build_field_chips)

        chips
      end

      def render_chip(label, remove_url)
        a(
          href: remove_url,
          class: "group inline-flex items-center gap-1 rounded-md bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-600 dark:text-gray-300 text-[10px] px-1.5 py-0.5 transition-colors",
          data: { turbo_frame: "_top" }
        ) do
          span(class: "truncate max-w-[150px]") { label }
          svg(
            class: "w-2.5 h-2.5 flex-shrink-0 text-gray-400 group-hover:text-gray-600 dark:group-hover:text-gray-300",
            fill: "none", stroke: "currentColor", viewBox: "0 0 24 24", aria_hidden: "true"
          ) { raw(safe(CLOSE_PATH)) }
        end
      end

      # One chip per (field_key, op) pair present in @filters.field_filters.
      # Only runs when @single_type is set (the view passes it in).
      def build_field_chips
        return [] unless @single_type && @filters.field_filters.any?

        schema = DocumentTypes::Schema.for(@single_type)
        fields_by_key = schema.fields.index_by { |f| f.key.to_s }
        chips = []

        @filters.field_filters.each do |field_key, ops|
          field = fields_by_key[field_key.to_s]
          next unless field

          ops.each do |op, raw_value|
            next if raw_value.to_s.blank?

            chips << {
              label: format_field_chip_label(field, op.to_s, raw_value.to_s),
              remove_path: remove_field_op(field_key, op)
            }
          end
        end

        chips
      end

      # Human-readable chip label for a single field filter op.
      def format_field_chip_label(field, op, raw_value)
        formatted = format_field_chip_value(field, op, raw_value)
        case op
        when "min"      then "#{field.label} ≥ #{formatted}"
        when "max"      then "#{field.label} ≤ #{formatted}"
        when "from"     then "#{field.label} #{t('.field_from', value: formatted)}"
        when "to"       then "#{field.label} #{t('.field_to', value: formatted)}"
        when "contains" then "#{field.label} \"#{raw_value}\""
        else                 "#{field.label}: #{formatted}"
        end
      end

      # Format the raw string value from params into a display string.
      def format_field_chip_value(field, _op, raw_value)
        case field.kind
        when :money
          euros = raw_value.to_f
          format_amount((euros * 100).round)
        when :date
          begin
            date = Date.parse(raw_value.to_s)
            l(date, format: :long)
          rescue ArgumentError, TypeError
            raw_value
          end
        when :boolean
          raw_value.to_s == "true" ? helpers.t("components.files.filter_panel.boolean_yes") : helpers.t("components.files.filter_panel.boolean_no")
        else
          raw_value.to_s
        end
      end

      # Remove URL for a single (field_key, op) pair from the field_filters hash.
      def remove_field_op(field_key, op)
        h = filter_hash
        # Deep-dup the nested :f hash so other chips are not affected.
        f = h.fetch(:f, {}).transform_values { |ops| ops.to_h.dup }
        field_ops = f[field_key.to_s].to_h.dup
        field_ops.delete(op.to_s)
        if field_ops.empty?
          f.delete(field_key.to_s)
        else
          f[field_key.to_s] = field_ops
        end
        if f.empty?
          h.delete(:f)
        else
          h[:f] = f
        end
        build_path(h)
      end

      # Build a removal URL for a single-value filter dimension.
      def remove_single(key)
        h = filter_hash
        h.delete(key)
        build_path(h)
      end

      # Build a removal URL for one element of an array filter.
      def remove_array_item(key, value)
        h = filter_hash
        arr = Array(h[key]).map(&:to_s).reject { |v| v == value.to_s }
        if arr.empty?
          h.delete(key)
        else
          h[key] = arr
        end
        build_path(h)
      end

      def filter_hash
        # to_h uses symbol keys; dup so we don't mutate across chips.
        h = @filters.to_h.transform_keys(&:to_sym)
        h[:q] = @q if @q.present?
        h
      end

      def build_path(h)
        if @folder
          helpers.files_folder_path(@folder, h)
        else
          helpers.files_path(h)
        end
      end

      def format_amount(cents)
        return "" unless cents
        helpers.format_currency(cents)
      end
    end
  end
end
