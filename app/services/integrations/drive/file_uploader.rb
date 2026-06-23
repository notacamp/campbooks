module Integrations
  module Drive
    # Uploads one or more files (from a FileSource) into a Drive folder.
    class FileUploader
      def initialize(account)
        @account = account
      end

      # files: array of Integrations::FileSource::Descriptor
      # Returns an array of OpenStruct(id:, name:, web_view_link:).
      def call(files:, folder_id: nil)
        client = ::GoogleDrive::Client.new(@account)
        Array(files).map do |f|
          f.open do |tempfile|
            client.upload_file(
              file_path: tempfile.path,
              file_name: f.filename,
              mime_type: f.content_type,
              parent_folder_id: folder_id.presence
            )
          end
        end
      end
    end
  end
end
