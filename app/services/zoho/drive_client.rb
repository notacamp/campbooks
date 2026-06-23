module Zoho
  class DriveClient
    BASE_URL = "https://workdrive.zoho.eu/api/v1"

    def initialize(drive_account)
      @drive_account = drive_account
      @oauth = OauthClient.new(refresh_token: drive_account.zoho_refresh_token)
    end

    def list_folders
      url = "#{BASE_URL}/folders"
      response = connection.get(url)
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? (data["data"] || []) : Array(data)
    end

    def get_folder(folder_id)
      url = "#{BASE_URL}/folders/#{folder_id}"
      response = connection.get(url)
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? data["data"] : data
    end

    def create_folder(name, parent_id = nil)
      url = "#{BASE_URL}/folders"
      payload = { name: name }
      payload[:parent_id] = parent_id if parent_id

      response = connection.post(url) do |req|
        req.headers["Content-Type"] = "application/json"
        req.body = payload.to_json
      end
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? data["data"] : data
    end

    def upload_file(file_path:, filename:, parent_id:)
      url = "#{BASE_URL}/upload"

      payload = {
        file: Faraday::Multipart::FilePart.new(file_path, "application/octet-stream", filename)
      }
      payload[:parent_id] = parent_id if parent_id

      response = connection.post(url) do |req|
        req.body = payload
      end
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? data["data"] : data
    rescue => e
      Rails.logger.error("[Zoho::DriveClient] upload_file failed: #{e.message}")
      nil
    end

    def get_file_info(file_id)
      url = "#{BASE_URL}/files/#{file_id}"
      response = connection.get(url)
      data = JSON.parse(response.body)
      data.is_a?(Hash) ? data["data"] : data
    end

    private

    def connection
      @connection ||= Faraday.new do |f|
        f.request :url_encoded
        f.request :multipart
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Zoho-oauthtoken #{@oauth.access_token}"
      end
    end
  end
end
