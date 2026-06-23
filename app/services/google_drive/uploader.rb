module GoogleDrive
  class Uploader
    def initialize(document)
      @document = document
      @config = document.classification&.google_drive_config
    end

    def call
      raise "No Google Drive config for document type '#{@document.document_type}'" unless @config
      account = @document.workspace.google_drive_accounts.connected.first
      raise "Google Drive not connected" unless account
      raise "No file to upload" unless @document.original_file.attached?

      client = Client.new(account)
      folder_id = FolderResolver.new(@document, @config, client).call
      filename = FilenameBuilder.new(@document, @config).call
      ext = File.extname(@document.original_file.filename.to_s)
      full_filename = "#{filename}#{ext}"

      tempfile = download_to_tempfile
      result = client.upload_file(
        file_path: tempfile.path,
        file_name: full_filename,
        mime_type: @document.original_file.content_type || "application/octet-stream",
        parent_folder_id: folder_id
      )

      @document.update!(
        google_drive_file_id: result.id,
        google_drive_push_status: :pushed,
        google_drive_pushed_at: Time.current,
        google_drive_push_error: nil
      )

      result
    rescue => e
      @document.update!(
        google_drive_push_status: :failed,
        google_drive_push_error: e.message
      )
      raise
    ensure
      tempfile&.close!
    end

    private

    def download_to_tempfile
      blob = @document.original_file.blob
      ext = File.extname(blob.filename.to_s)
      tempfile = Tempfile.new([ "drive_upload", ext ])
      tempfile.binmode
      blob.download { |chunk| tempfile.write(chunk) }
      tempfile.rewind
      tempfile
    end
  end
end
