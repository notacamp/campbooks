module Notion
  class PushService
    def initialize(document, mapping = nil)
      @document = document
      @mapping = mapping || document.notion_database_mapping
    end

    def call
      return failure("No Notion integration configured") unless integration
      return failure("No database mapping for this document type") unless @mapping
      return failure("Push not enabled for this document type") unless @mapping.push_enabled

      existing = NotionPage.find_by(document_id: @document.id)

      if existing&.synced?
        return { success: true, notion_page_id: existing.notion_page_id, action: :skipped }
      end

      client = Client.new(integration)
      properties = FieldMapper.to_notion_properties(@document, @mapping)

      if existing&.outdated? || existing&.error?
        # Update existing Notion page
        client.update_page(existing.notion_page_id, properties: properties)
        existing.update!(sync_status: :synced, last_synced_at: Time.current, last_error: nil)
        { success: true, notion_page_id: existing.notion_page_id, action: :updated }
      else
        # Create new page
        children = build_file_blocks
        page = client.create_page(@mapping.notion_database_id, properties: properties, children: children)

        if page["id"]
          NotionPage.create!(
            document: @document,
            notion_database_mapping: @mapping,
            notion_page_id: page["id"],
            last_synced_at: Time.current,
            sync_status: :synced
          )
          { success: true, notion_page_id: page["id"], action: :created }
        else
          error_msg = page["message"] || page["error"] || "Unknown Notion API error"
          raise error_msg
        end
      end
    rescue => e
      if existing
        existing.update!(sync_status: :error, last_error: e.message)
      end
      failure(e.message)
    end

    private

    def integration
      @integration ||= @document.workspace.notion_integrations.active.first
    end

    def build_file_blocks
      return [] unless @document.original_file.attached?

      file_url = Rails.application.routes.url_helpers.rails_blob_url(
        @document.original_file,
        host: ENV.fetch("APP_HOST", "localhost:3000"),
        protocol: ENV.fetch("APP_PROTOCOL", "http")
      )

      [
        {
          type: "file",
          file: {
            type: "external",
            name: @document.original_file.filename.to_s,
            external: { url: file_url }
          }
        }
      ]
    end

    def failure(message)
      Rails.logger.error("[Notion::PushService] #{message}")
      { success: false, error: message, action: :error }
    end
  end
end
