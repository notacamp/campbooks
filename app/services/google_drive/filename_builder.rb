module GoogleDrive
  class FilenameBuilder
    PLACEHOLDERS = %w[{date} {entity} {reference} {type} {vendor} {id}].freeze

    def initialize(document, config)
      @document = document
      @config = config
    end

    def call
      pattern = @config.naming_pattern
      pattern.dup.tap do |str|
        replacements.each { |key, value| str.gsub!(key, value.to_s) }
      end
    end

    private

    def replacements
      {
        "{date}" => formatted_date,
        "{entity}" => sanitize(@document.entity_display_name),
        "{reference}" => sanitize(@document.reference_display),
        "{type}" => sanitize(@document.classification&.name || @document.document_type),
        "{vendor}" => sanitize(@document.vendor_name),
        "{id}" => @document.id.to_s
      }
    end

    def formatted_date
      @document.document_date&.strftime("%Y%m%d") || Date.current.strftime("%Y%m%d")
    end

    def sanitize(str)
      return "unknown" if str.blank?
      str.to_s
         .gsub(/[^a-zA-Z0-9\-_\.]/, "_")
         .gsub(/_+/, "_")
         .gsub(/\A_|_\z/, "")
         .truncate(60, omission: "")
    end
  end
end
