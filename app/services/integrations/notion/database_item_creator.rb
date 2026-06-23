module Integrations
  module Notion
    # Creates a row (page) in a Notion database from a map of property values, with
    # files uploaded into "files" properties. Use ::Notion (top-level) for the API
    # client so the enclosing Integrations::Notion namespace doesn't shadow it.
    class DatabaseItemCreator
      def initialize(integration)
        @integration = integration
      end

      # database_id: target database
      # inputs:      { "Property Name" => { type:, value: } }
      # file_fields: { "Files Property" => [FileSource::Descriptor, ...] }
      # Returns the created page hash from the Notion API.
      def call(database_id:, inputs:, file_fields: {})
        uploaded = upload_files(file_fields)
        properties = ::Notion::PropertyBuilder.build(inputs, file_uploads: uploaded)
        ::Notion::Client.new(@integration).create_page(database_id, properties: properties)
      end

      private

      def upload_files(file_fields)
        uploader = ::Notion::FileUploader.new(@integration)
        (file_fields || {}).each_with_object({}) do |(prop, descriptors), acc|
          files = Array(descriptors).map do |f|
            id = f.open { |io| uploader.upload(io: io, filename: f.filename, content_type: f.content_type) }
            { id: id, name: f.filename }
          end
          acc[prop] = files if files.any?
        end
      end
    end
  end
end
