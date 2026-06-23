module Documents
  # Interactive "Send this document to Notion": pick a connected workspace, then
  # either add a row to a database (with a form built from the database schema and
  # the file uploaded to a files property) or create a subpage under a chosen page.
  class NotionExportsController < ApplicationController
    before_action :set_document
    before_action :load_integrations
    before_action :set_integration, only: [ :databases, :database_form, :pages ]

    # GET .../notion_export/new?integration_id=
    def new
      @integration = @integrations.find_by(id: params[:integration_id]) || @integrations.first
    end

    # GET .../notion_export/databases (Turbo Frame)
    def databases
      result = client.list_databases(query: params[:q])
      @databases = (result["results"] || []).reject { |d| d["archived"] }
    rescue => e
      @error = e.message
    end

    # GET .../notion_export/database_form?database_id= (Turbo Frame)
    def database_form
      @database_id = params[:database_id]
      @schema = client.get_database(@database_id)
      @database_title = notion_title(@schema)
    rescue => e
      @error = e.message
    end

    # GET .../notion_export/pages (Turbo Frame)
    def pages
      result = client.list_pages(query: params[:q])
      @pages = (result["results"] || []).reject { |p| p["archived"] }
    rescue => e
      @error = e.message
    end

    # POST .../notion_export
    def create
      integration = @integrations.find(params[:integration_id])

      page =
        case params[:kind]
        when "database" then create_database_item(integration)
        when "page" then create_subpage(integration)
        else
          return redirect_to new_document_notion_export_path(@document), error: t(".pick_destination")
        end

      if page && page["id"]
        redirect_to @document, success: t(".created")
      else
        redirect_to new_document_notion_export_path(@document, integration_id: integration.id),
                    error: (page && (page["message"] || page["error"])) || t(".failed")
      end
    rescue => e
      redirect_to new_document_notion_export_path(@document), error: t(".failed_with", message: e.message)
    end

    private

    def create_database_item(integration)
      database_id = params[:database_id]
      schema = ::Notion::Client.new(integration).get_database(database_id)
      inputs = build_inputs(schema, params.dig(:notion, :properties))
      file_fields = build_file_fields(params.dig(:notion, :file_props))
      Integrations::Notion::DatabaseItemCreator.new(integration)
        .call(database_id: database_id, inputs: inputs, file_fields: file_fields)
    end

    def create_subpage(integration)
      files = params[:attach_file] == "1" ? Integrations::FileSource.for(document: @document) : []
      Integrations::Notion::PageCreator.new(integration).call(
        parent_page_id: params[:parent_page_id],
        title: params[:title].presence || @document.display_title,
        content: params[:note],
        files: files
      )
    end

    # Re-read the schema to learn each property's type — never trust the client.
    def build_inputs(schema, submitted)
      return {} if submitted.blank?
      props = schema["properties"] || {}
      submitted.to_unsafe_h.each_with_object({}) do |(name, value), acc|
        type = props.dig(name, "type")
        next unless type
        acc[name] = { type: type, value: value }
      end
    end

    def build_file_fields(file_props)
      return {} if file_props.blank?
      descriptors = Integrations::FileSource.for(document: @document)
      return {} if descriptors.empty?
      file_props.to_unsafe_h
        .select { |_name, v| %w[1 true on].include?(v.to_s) }
        .keys.index_with { descriptors }
    end

    def set_document
      @document = Current.workspace.documents.find(params[:document_id])
    end

    def load_integrations
      @integrations = Current.workspace.notion_integrations.active.order(:created_at)
      if @integrations.empty?
        redirect_to settings_integrations_notion_path,
                    error: t("documents.notion_exports.errors.not_connected")
      end
    end

    def set_integration
      @integration = @integrations.find(params[:integration_id])
    end

    def client
      @client ||= ::Notion::Client.new(@integration)
    end

    def notion_title(schema)
      (schema["title"] || []).map { |t| t.dig("plain_text") || t.dig("text", "content") }.join.presence
    end
  end
end
