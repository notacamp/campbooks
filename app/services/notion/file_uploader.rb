require "faraday/multipart"

module Notion
  # Uploads a file to Notion via the File Upload API and returns the resulting
  # `file_upload` id, which can then be referenced from a "files" page property or
  # a file block. Files up to 20 MB go in a single request; larger files are split
  # into multi-part chunks (streamed from the IO, so memory stays bounded).
  #
  #   id = Notion::FileUploader.new(integration).upload(io:, filename:, content_type:)
  #   props["Attachment"] = Notion::FileUploader.files_property([id], names: [filename])
  class FileUploader
    BASE_URL = Notion::Client::BASE_URL
    API_VERSION = Notion::Client::API_VERSION
    MAX_SINGLE_PART_BYTES = 20 * 1024 * 1024 # Notion's single-request limit
    PART_SIZE = 10 * 1024 * 1024             # within Notion's 5–20 MB per-part range

    class Error < StandardError; end

    def initialize(integration)
      @integration = integration
    end

    # io: any readable IO (Tempfile/StringIO/File) or a path String.
    # Returns the file_upload id.
    def upload(io:, filename:, content_type: "application/octet-stream")
      io = File.open(io, "rb") if io.is_a?(String)
      size = io.size if io.respond_to?(:size)

      if size && size > MAX_SINGLE_PART_BYTES
        upload_multi_part(io, filename, content_type, size)
      else
        upload_single_part(io, filename, content_type)
      end
    end

    # Builds a "files" property value referencing already-uploaded file ids.
    def self.files_property(ids, names: [])
      {
        "files" => Array(ids).each_with_index.map do |id, i|
          { "type" => "file_upload", "name" => (names[i].presence || "file"), "file_upload" => { "id" => id } }
        end
      }
    end

    # Builds a file block (for page content / children).
    def self.file_block(id, name: "file")
      { "type" => "file", "file" => { "type" => "file_upload", "file_upload" => { "id" => id }, "name" => name } }
    end

    private

    def upload_single_part(io, filename, content_type)
      data = io.read
      raise Error, "#{filename} exceeds Notion's 20 MB single-upload limit" if data.bytesize > MAX_SINGLE_PART_BYTES

      id = create_file_upload(filename: filename, content_type: content_type)["id"]
      raise Error, "Notion file upload init failed" unless id

      sent = send_part(id, data, filename, content_type)
      raise Error, (sent["message"] || "Notion file upload did not complete") unless sent["status"] == "uploaded"
      id
    end

    def upload_multi_part(io, filename, content_type, size)
      number_of_parts = (size.to_f / PART_SIZE).ceil
      id = create_file_upload(filename: filename, content_type: content_type,
                              mode: "multi_part", number_of_parts: number_of_parts)["id"]
      raise Error, "Notion file upload init failed" unless id

      part_number = 0
      while (chunk = io.read(PART_SIZE))
        part_number += 1
        send_part(id, chunk, filename, content_type, part_number: part_number)
      end

      completed = complete_file_upload(id)
      raise Error, (completed["message"] || "Notion file upload did not complete") unless completed["status"] == "uploaded"
      id
    end

    def create_file_upload(filename:, content_type:, mode: "single_part", number_of_parts: nil)
      body = { mode: mode, filename: filename, content_type: content_type }
      body[:number_of_parts] = number_of_parts if number_of_parts

      response = json_connection.post("#{BASE_URL}/file_uploads") { |req| req.body = body.to_json }
      JSON.parse(response.body)
    rescue JSON::ParserError
      { "message" => "Unparseable response from Notion file_uploads" }
    end

    def send_part(id, data, filename, content_type, part_number: nil)
      part = Faraday::Multipart::FilePart.new(StringIO.new(data), content_type, filename)
      payload = { file: part }
      payload[:part_number] = part_number.to_s if part_number

      response = multipart_connection.post("#{BASE_URL}/file_uploads/#{id}/send") { |req| req.body = payload }
      JSON.parse(response.body)
    rescue JSON::ParserError
      { "message" => "Unparseable response from Notion file_uploads/send" }
    end

    def complete_file_upload(id)
      response = json_connection.post("#{BASE_URL}/file_uploads/#{id}/complete")
      JSON.parse(response.body)
    rescue JSON::ParserError
      { "message" => "Unparseable response from Notion file_uploads/complete" }
    end

    def json_connection
      @json_connection ||= Faraday.new do |f|
        f.use SystemHealth::FaradayMiddleware, service: "notion", workspace: -> { @integration.try(:workspace_id) }
        f.request :json
        f.response :raise_error
        f.options.open_timeout = 10
        f.options.timeout = 60
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Bearer #{@integration.access_token}"
        f.headers["Notion-Version"] = API_VERSION
      end
    end

    def multipart_connection
      @multipart_connection ||= Faraday.new do |f|
        f.use SystemHealth::FaradayMiddleware, service: "notion", workspace: -> { @integration.try(:workspace_id) }
        f.request :multipart
        f.response :raise_error
        f.options.open_timeout = 10
        # File transfers can be large; bound but generous (mirrors GoogleDrive::Client).
        f.options.timeout = 120
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Bearer #{@integration.access_token}"
        f.headers["Notion-Version"] = API_VERSION
      end
    end
  end
end
