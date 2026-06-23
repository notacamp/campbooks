module GoogleDrive
  class Client
    BASE_URL = "https://www.googleapis.com/drive/v3"
    UPLOAD_URL = "https://www.googleapis.com/upload/drive/v3"

    def initialize(account)
      @account = account
      raise "Google Drive not connected" unless @account&.connected?
      @oauth_client = OauthClient.new
    end

    def upload_file(file_path:, file_name:, mime_type:, parent_folder_id: nil)
      metadata = { name: file_name, mimeType: mime_type }
      metadata[:parents] = [ parent_folder_id ] if parent_folder_id.present?

      boundary = "campbooks_drive_#{SecureRandom.hex(16)}"
      file_content = File.binread(file_path)

      body = "--#{boundary}\r\n"
      body += "Content-Type: application/json; charset=UTF-8\r\n\r\n"
      body += "#{metadata.to_json}\r\n"
      body += "--#{boundary}\r\n"
      body += "Content-Type: #{mime_type}\r\n"
      body += "Content-Transfer-Encoding: binary\r\n\r\n"
      body += file_content
      body += "\r\n--#{boundary}--"

      response = raw_connection.post("#{UPLOAD_URL}/files") do |req|
        req.params["uploadType"] = "multipart"
        req.params["fields"] = "id,name,webViewLink"
        req.headers["Content-Type"] = "multipart/related; boundary=#{boundary}"
        req.body = body
      end

      data = JSON.parse(response.body)
      raise ApiError, data.dig("error", "message") || "Upload failed" if data["error"]
      OpenStruct.new(id: data["id"], name: data["name"], web_view_link: data["webViewLink"])
    end

    def create_folder(name, parent_folder_id: nil)
      metadata = { name: name, mimeType: "application/vnd.google-apps.folder" }
      metadata[:parents] = [ parent_folder_id ] if parent_folder_id.present?

      response = json_connection.post("#{BASE_URL}/files") do |req|
        req.params["fields"] = "id,name"
        req.body = metadata.to_json
      end

      data = JSON.parse(response.body)
      raise ApiError, data.dig("error", "message") || "Folder creation failed" if data["error"]
      OpenStruct.new(id: data["id"], name: data["name"])
    end

    def find_folder_by_name(name, parent_folder_id: nil)
      query = "mimeType='application/vnd.google-apps.folder' and name='#{escape_query(name)}' and trashed=false"
      query += " and '#{parent_folder_id}' in parents" if parent_folder_id.present?

      response = json_connection.get("#{BASE_URL}/files") do |req|
        req.params["q"] = query
        req.params["fields"] = "files(id,name)"
      end

      data = JSON.parse(response.body)
      file = data["files"]&.first
      file ? OpenStruct.new(id: file["id"], name: file["name"]) : nil
    end

    def find_or_create_folder(path_segments, root_folder_id: nil)
      current_parent = root_folder_id
      path_segments.each do |segment|
        folder = find_folder_by_name(segment, parent_folder_id: current_parent)
        folder = create_folder(segment, parent_folder_id: current_parent) unless folder
        current_parent = folder.id
      end
      current_parent
    end

    # Lists child folders for the interactive folder browser. Requires the full
    # `drive` scope (the legacy `drive.file` scope only surfaces app-created files).
    # Defaults to My Drive root; pass a folder id to descend.
    def list_folders(parent_id: nil, query: nil)
      parent = parent_id.presence || "root"
      q = "mimeType='application/vnd.google-apps.folder' and trashed=false and '#{parent}' in parents"
      q += " and name contains '#{escape_query(query)}'" if query.present?

      response = json_connection.get("#{BASE_URL}/files") do |req|
        req.params["q"] = q
        req.params["fields"] = "files(id,name)"
        req.params["orderBy"] = "name"
        req.params["pageSize"] = 200
        req.params["spaces"] = "drive"
      end

      data = JSON.parse(response.body)
      raise ApiError, data.dig("error", "message") || "Folder listing failed" if data["error"]
      (data["files"] || []).map { |f| OpenStruct.new(id: f["id"], name: f["name"]) }
    end

    # Fetches a single folder's id + name (used to label a pasted/selected folder).
    def get_folder(folder_id)
      response = json_connection.get("#{BASE_URL}/files/#{folder_id}") do |req|
        req.params["fields"] = "id,name,mimeType"
      end
      data = JSON.parse(response.body)
      return nil if data["error"]
      OpenStruct.new(id: data["id"], name: data["name"])
    rescue Faraday::Error
      nil
    end

    private

    def json_connection
      @json_connection ||= Faraday.new do |f|
        f.request :json
        f.response :raise_error
        # Bound every call so a hung Drive response can't wedge the worker.
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Bearer #{access_token}"
        f.headers["Content-Type"] = "application/json"
      end
    end

    def raw_connection
      @raw_connection ||= Faraday.new do |f|
        f.response :raise_error
        # Raw file transfers can be large, so a generous read bound — but still
        # finite, so a stalled transfer can't pin the worker open indefinitely.
        f.options.open_timeout = 10
        f.options.timeout = 120
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Bearer #{access_token}"
      end
    end

    def access_token
      @access_token ||= @oauth_client.refresh_access_token(@account.refresh_token)
    end

    def escape_query(str)
      str.gsub("'", "\\'")
    end
  end

  class ApiError < StandardError; end
end
