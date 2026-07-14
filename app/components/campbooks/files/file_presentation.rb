# frozen_string_literal: true

module Campbooks
  module Files
    # Shared presentation for a file (Document) across FileRow and FileCard: a
    # content-type icon, a short kind label, and a human-readable size. Mixed into
    # Campbooks::Base subclasses — relies on @doc plus the Base helpers (t/helpers/
    # safe). Uses absolute i18n keys so the label resolves the same regardless of
    # which component's scope it's mixed into.
    module FilePresentation
      FILE_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-[18px] w-[18px]"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/></svg>'
      PDF_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-[18px] w-[18px]"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5M9 13h2a1.5 1.5 0 0 1 0 3H9zM9 13v6"/></svg>'
      IMAGE_ICON = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="h-[18px] w-[18px]"><rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="8.5" cy="9.5" r="1.5"/><path d="m21 16-5-5L5 20"/></svg>'

      def type_icon
        return IMAGE_ICON if @doc.image?
        return PDF_ICON if @doc.pdf?

        FILE_ICON
      end

      def kind_label
        return t("components.files.file_meta.kind_image") if @doc.image?
        return "PDF" if @doc.pdf?

        ext = File.extname(@doc.original_file.filename.to_s).delete(".").upcase
        ext.presence || t("components.files.file_meta.kind_file")
      end

      def human_size
        return "—" unless @doc.original_file.attached?

        helpers.number_to_human_size(@doc.original_file.byte_size)
      end

      # The first money/date schema field's formatted value (single-type filtered
      # views pass `field_columns:`), or nil. Appended to FileCard/FileTile meta
      # lines so the compact layouts still surface the column that matters.
      def primary_field_value(field_columns)
        field = Array(field_columns).find { |f| f.kind == :money || f.kind == :date }
        return nil unless field

        value = field.read(@doc.metadata)
        return nil unless value

        case field.kind
        when :money then helpers.format_currency(value)
        when :date  then helpers.l(value, format: :long)
        end
      end
    end
  end
end
