module Integrations
  module Drive
    # Creates a folder in Google Drive (optionally nested under a parent).
    class FolderCreator
      def initialize(account)
        @account = account
      end

      # Returns an OpenStruct(id:, name:).
      def call(name:, parent_id: nil)
        ::GoogleDrive::Client.new(@account).create_folder(name, parent_folder_id: parent_id.presence)
      end
    end
  end
end
