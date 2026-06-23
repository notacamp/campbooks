module Notion
  class Client
    BASE_URL = "https://api.notion.com/v1"
    API_VERSION = "2022-06-28"

    def initialize(integration)
      @integration = integration
    end

    # --- Databases ---

    def list_databases(query: nil, start_cursor: nil)
      payload = { page_size: 100 }
      payload[:query] = query if query.present?
      payload[:start_cursor] = start_cursor if start_cursor.present?

      response = connection.post("#{BASE_URL}/search") do |req|
        req.body = payload.merge(filter: { property: "object", value: "database" }).to_json
      end

      parse_response(response)
    end

    def get_database(id)
      response = connection.get("#{BASE_URL}/databases/#{id}")
      parse_response(response)
    end

    # Generic search across the connected workspace. Pass a filter to restrict to
    # pages or databases: { property: "object", value: "page" | "database" }.
    def search(query: nil, filter: nil, start_cursor: nil, page_size: 100)
      payload = { page_size: page_size }
      payload[:query] = query if query.present?
      payload[:filter] = filter if filter
      payload[:start_cursor] = start_cursor if start_cursor

      response = connection.post("#{BASE_URL}/search") do |req|
        req.body = payload.to_json
      end

      parse_response(response)
    end

    # Search pages only (for the subpage parent picker).
    def list_pages(query: nil, start_cursor: nil)
      search(query: query, start_cursor: start_cursor, filter: { property: "object", value: "page" })
    end

    def query_database(id, filter: nil, sorts: nil, start_cursor: nil, page_size: 100)
      payload = { page_size: page_size }
      payload[:filter] = filter if filter
      payload[:sorts] = sorts if sorts
      payload[:start_cursor] = start_cursor if start_cursor

      response = connection.post("#{BASE_URL}/databases/#{id}/query") do |req|
        req.body = payload.to_json
      end

      parse_response(response)
    end

    # --- Pages ---

    def create_page(database_id, properties:, children: nil, icon: nil)
      create_page_under({ database_id: database_id }, properties: properties, children: children, icon: icon)
    end

    # Generalized page creation. `parent` is { database_id: id } to add a row to a
    # database, or { page_id: id } to create a subpage nested under another page.
    def create_page_under(parent, properties: {}, children: nil, icon: nil)
      payload = { parent: build_parent(parent), properties: properties }
      payload[:children] = children if children.present?
      payload[:icon] = icon if icon

      response = connection.post("#{BASE_URL}/pages") do |req|
        req.body = payload.to_json
      end

      parse_response(response)
    end

    def get_page(id)
      response = connection.get("#{BASE_URL}/pages/#{id}")
      parse_response(response)
    end

    def update_page(id, properties:)
      response = connection.patch("#{BASE_URL}/pages/#{id}") do |req|
        req.body = { properties: properties }.to_json
      end

      parse_response(response)
    end

    # --- Blocks ---

    def get_block_children(block_id, start_cursor: nil)
      params = { page_size: 100 }
      params[:start_cursor] = start_cursor if start_cursor

      response = connection.get("#{BASE_URL}/blocks/#{block_id}/children") do |req|
        params.each { |k, v| req.params[k] = v }
      end

      parse_response(response)
    end

    def append_blocks(block_id, blocks:)
      response = connection.patch("#{BASE_URL}/blocks/#{block_id}/children") do |req|
        req.body = { children: blocks }.to_json
      end

      parse_response(response)
    end

    # --- Users ---

    def get_bot_info
      response = connection.get("#{BASE_URL}/users/me")
      parse_response(response)
    end

    private

    def connection
      @connection ||= Faraday.new do |f|
        f.request :json
        f.response :raise_error
        # Bound every call so a hung Notion response can't wedge the sync worker
        # (NotionSyncJob runs on the same shared worker as the email/calendar scans).
        f.options.open_timeout = 10
        f.options.timeout = 30
        f.adapter Faraday.default_adapter
        f.headers["Authorization"] = "Bearer #{@integration.access_token}"
        f.headers["Notion-Version"] = API_VERSION
        f.headers["Content-Type"] = "application/json"
      end
    end

    def build_parent(parent)
      parent = parent.with_indifferent_access
      if parent[:database_id].present?
        { type: "database_id", database_id: parent[:database_id] }
      elsif parent[:page_id].present?
        { type: "page_id", page_id: parent[:page_id] }
      else
        raise ArgumentError, "parent must include :database_id or :page_id"
      end
    end

    def parse_response(response)
      JSON.parse(response.body)
    rescue JSON::ParserError
      { "error" => response.body.to_s }
    end
  end
end
