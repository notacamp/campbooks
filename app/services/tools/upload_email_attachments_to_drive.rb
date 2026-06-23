module Tools
  # Uploads the triggering email's attachments to the workspace's Google Drive.
  # Optional args[:folder_name] places them in a named folder (created at My Drive
  # root if missing). Returns a result hash on success, nil on failure — matching
  # the other Tools::* services so EmailActions can wrap it.
  class UploadEmailAttachmentsToDrive
    def self.call(email_message, args = {}, user: Current.user)
      new(email_message, args, user).call
    end

    def initialize(email_message, args, user)
      @email = email_message
      @args = (args || {})
      @user = user
    end

    def call
      return nil unless @email

      account = @email.email_account&.workspace&.google_drive_accounts&.connected&.first
      return nil unless account

      files = Integrations::FileSource.for(email_message: @email)
      return nil if files.empty?

      folder_id = resolve_folder_id(account)
      results = Integrations::Drive::FileUploader.new(account).call(files: files, folder_id: folder_id)
      { count: results.size, folder_name: @args[:folder_name].presence }
    end

    private

    def resolve_folder_id(account)
      name = @args[:folder_name].to_s.strip
      return nil if name.blank?
      ::GoogleDrive::Client.new(account).find_or_create_folder([ name ])
    end
  end
end
