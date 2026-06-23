module GoogleDrive
  class FolderResolver
    def initialize(document, config, client)
      @document = document
      @config = config
      @client = client
    end

    def call
      return @config.folder_id if @config.subfolder_pattern == "flat"

      base_folder_id = @config.folder_id
      subfolder_segments = build_subfolder_segments

      if subfolder_segments.any?
        @client.find_or_create_folder(subfolder_segments, root_folder_id: base_folder_id)
      else
        base_folder_id
      end
    end

    private

    def build_subfolder_segments
      case @config.subfolder_pattern
      when "year"
        [ year_string ]
      when "year_month"
        date = @document.document_date || Date.current
        [ date.year.to_s, date.strftime("%m_%B") ]
      when "entity"
        name = sanitize(@document.entity_display_name)
        name.present? ? [ name ] : []
      else
        []
      end
    end

    def year_string
      (@document.document_date&.year || Date.current.year).to_s
    end

    def sanitize(str)
      return nil if str.blank?
      str.to_s.gsub(/[^a-zA-Z0-9\-_ ]/, "_").gsub(/_+/, "_").truncate(40, omission: "")
    end
  end
end
