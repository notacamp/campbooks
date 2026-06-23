module Documents
  class FilenameGenerator
    def initialize(document)
      @document = document
    end

    # entity_date_reference, e.g. "empresa_exemplo_lda_20250115_ft2025_0042.pdf".
    # Statements (which have a period rather than a single date + invoice number)
    # become "bank_name_20250101_to_20250131.pdf".
    def call
      parts = [
        sanitize(@document.entity_display_name),
        date_segment,
        # The period is already the date segment for statements — don't repeat it
        # as the reference (reference_display returns the period for those).
        period_dates.all? ? nil : sanitize(@document.reference_display)
      ].compact.reject(&:blank?)

      ext = file_extension

      if parts.empty?
        "document_#{@document.id}_#{Time.current.strftime('%Y%m%d%H%M%S')}#{ext}"
      else
        "#{parts.join('_')}#{ext}"
      end
    end

    private

    # A statement period ("YYYYMMDD_to_YYYYMMDD") when both ends are known,
    # otherwise the single document date ("YYYYMMDD").
    def date_segment
      ps, pe = period_dates
      if ps && pe
        "#{ps.strftime('%Y%m%d')}_to_#{pe.strftime('%Y%m%d')}"
      else
        @document.document_date&.strftime("%Y%m%d")
      end
    end

    # Period from metadata first, then columns (mirrors Document#reference_display).
    def period_dates
      @period_dates ||= begin
        m = @document.metadata.presence || {}
        [ parse_date(m["period_start"] || @document.period_start),
          parse_date(m["period_end"]   || @document.period_end) ]
      end
    end

    def parse_date(value)
      return nil if value.blank?
      return value if value.respond_to?(:strftime)
      Date.parse(value.to_s)
    rescue ArgumentError, TypeError
      nil
    end

    def file_extension
      orig = @document.original_file.filename.to_s
      ext = File.extname(orig)
      ext.presence || ".pdf"
    end

    def sanitize(str)
      return nil if str.blank?

      str.unicode_normalize(:nfkd)
         .gsub(/[^\x00-\x7F]/, "")
         .gsub(/[^a-zA-Z0-9\-]/, "_")
         .gsub(/_+/, "_")
         .gsub(/\A_|_\z/, "")
         .downcase
         .truncate(50, omission: "")
    end
  end
end
