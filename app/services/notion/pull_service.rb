module Notion
  class PullService
    def initialize(mapping)
      @mapping = mapping
    end

    def call
      return failure("No Notion integration configured") unless integration
      return failure("Pull not enabled for this document type") unless @mapping.pull_enabled

      client = Client.new(integration)
      stats = { pages_scanned: 0, created: 0, skipped: 0, errors: 0 }
      start_cursor = nil

      loop do
        result = client.query_database(@mapping.notion_database_id, start_cursor: start_cursor)
        break unless result["results"].is_a?(Array)

        result["results"].each do |page|
          process_page(client, page, stats)
        end

        break unless result["has_more"] && result["next_cursor"]
        start_cursor = result["next_cursor"]
      end

      stats
    rescue => e
      failure(e.message)
    end

    private

    def integration
      @integration ||= @mapping.document_type.workspace.notion_integrations.active.first
    end

    def process_page(client, page_data, stats)
      page_id = page_data["id"]
      stats[:pages_scanned] += 1

      if NotionPage.exists?(notion_page_id: page_id)
        stats[:skipped] += 1
        return
      end

      properties = page_data["properties"] || {}
      title = extract_title(properties)

      # Extract metadata from page properties
      metadata = FieldMapper.from_notion_page(page_data, @mapping)

      # Download file attachments
      file_blocks = download_file_blocks(client, page_id)

      ActiveRecord::Base.transaction do
        document = Document.create!(
          document_type: @mapping.document_type.name,
          ai_status: :completed,
          review_status: :approved,
          source: :notion,
          workspace: @mapping.document_type.workspace,
          metadata: metadata,
          description: "Imported from Notion: #{title}"
        )

        file_blocks.each do |block|
          attach_file_to_document(document, block)
        end

        NotionPage.create!(
          document: document,
          notion_database_mapping: @mapping,
          notion_page_id: page_id,
          last_synced_at: Time.current,
          sync_status: :synced
        )

        stats[:created] += 1
      end
    rescue ActiveRecord::RecordInvalid => e
      Rails.logger.error("[Notion::PullService] Failed to create document from page #{page_id}: #{e.message}")
      stats[:errors] += 1
    end

    def extract_title(properties)
      title_prop = properties.values.find { |v| v["type"] == "title" }
      title_prop&.dig("title", 0, "text", "content") || "Untitled"
    end

    def download_file_blocks(client, page_id)
      result = client.get_block_children(page_id)
      blocks = result["results"] || []

      blocks.select { |b| b["type"] == "file" }
    end

    def attach_file_to_document(document, file_block)
      file_data = file_block["file"]
      return unless file_data

      url = nil
      filename = "untitled"

      case file_data["type"]
      when "external"
        url = file_data.dig("external", "url")
        filename = file_data["name"] || "document"
      when "file"
        url = file_data["url"]
        filename = file_data["name"] || "document"
      end

      return unless url

      io = URI.parse(url).open
      document.original_file.attach(io: io, filename: filename)
    rescue => e
      Rails.logger.error("[Notion::PullService] Failed to download file: #{e.message}")
    end

    def failure(message)
      Rails.logger.error("[Notion::PullService] #{message}")
      { success: false, error: message, pages_scanned: 0, created: 0, skipped: 0, errors: 0 }
    end
  end
end
