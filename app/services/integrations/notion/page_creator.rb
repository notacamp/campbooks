module Integrations
  module Notion
    # Creates a Notion page nested under another page (a "subpage"), optionally with
    # body text and uploaded file blocks. Use ::Notion (top-level) for the API client
    # so the enclosing Integrations::Notion namespace doesn't shadow it.
    class PageCreator
      def initialize(integration)
        @integration = integration
      end

      # Returns the created page hash from the Notion API.
      def call(parent_page_id:, title:, content: nil, files: [])
        properties = { "title" => { "title" => [ { "text" => { "content" => title.to_s } } ] } }

        children = []
        children << paragraph_block(content) if content.present?
        Array(files).each do |f|
          id = f.open do |io|
            ::Notion::FileUploader.new(@integration).upload(io: io, filename: f.filename, content_type: f.content_type)
          end
          children << ::Notion::FileUploader.file_block(id, name: f.filename)
        end

        ::Notion::Client.new(@integration).create_page_under(
          { page_id: parent_page_id },
          properties: properties,
          children: children.presence
        )
      end

      private

      def paragraph_block(text)
        {
          "object" => "block",
          "type" => "paragraph",
          "paragraph" => { "rich_text" => [ { "type" => "text", "text" => { "content" => text.to_s } } ] }
        }
      end
    end
  end
end
