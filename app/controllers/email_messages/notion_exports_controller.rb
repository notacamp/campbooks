module EmailMessages
  # Interactive "Save email to Notion": pick a connected workspace, then add a row
  # to a database (form built from the schema; the email's attachments upload to a
  # files property) or create a subpage under a chosen page. Mirrors the document
  # flow with the email as the subject and its attachments as the file source.
  class NotionExportsController < ApplicationController
    before_action :set_email_message
    before_action :load_integrations
    before_action :set_integration, only: [ :databases, :database_form, :pages ]

    def new
      @integration = @integrations.find_by(id: params[:integration_id]) || @integrations.first
    end

    def databases
      result = client.list_databases(query: params[:q])
      @databases = (result["results"] || []).reject { |d| d["archived"] }
    rescue => e
      @error = e.message
    end

    def database_form
      @database_id = params[:database_id]
      @schema = client.get_database(@database_id)
      @database_title = notion_title(@schema)
    rescue => e
      @error = e.message
    end

    def pages
      result = client.list_pages(query: params[:q])
      @pages = (result["results"] || []).reject { |p| p["archived"] }
    rescue => e
      @error = e.message
    end

    def create
      integration = @integrations.find(params[:integration_id])

      page =
        case params[:kind]
        when "database" then create_database_item(integration)
        when "page" then create_subpage(integration)
        else
          return redirect_to new_email_message_notion_export_path(@email_message), error: t(".pick_destination")
        end

      if page && page["id"]
        redirect_to email_message_path(@email_message), success: t(".created")
      else
        redirect_to new_email_message_notion_export_path(@email_message, integration_id: integration.id),
                    error: (page && (page["message"] || page["error"])) || t(".failed")
      end
    rescue => e
      redirect_to new_email_message_notion_export_path(@email_message), error: t(".failed_with", message: e.message)
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
      files = params[:attach_file] == "1" ? email_files : []
      Integrations::Notion::PageCreator.new(integration).call(
        parent_page_id: params[:parent_page_id],
        title: params[:title].presence || @email_message.subject.presence || "(no subject)",
        content: params[:note],
        files: files
      )
    end

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
      files = email_files
      return {} if files.empty?
      file_props.to_unsafe_h
        .select { |_name, v| %w[1 true on].include?(v.to_s) }
        .keys.index_with { files }
    end

    def email_files
      Integrations::FileSource.for(email_message: @email_message)
    end

    def set_email_message
      @email_message = EmailMessage.accessible_to(Current.user).find(params[:email_message_id])
    end

    def load_integrations
      @integrations = Current.workspace.notion_integrations.active.order(:created_at)
      if @integrations.empty?
        redirect_to settings_integrations_notion_path,
                    error: t("email_messages.notion_exports.errors.not_connected")
      end
    end

    def set_integration
      @integration = @integrations.find(params[:integration_id])
    end

    def client
      @client ||= ::Notion::Client.new(@integration)
    end

    def notion_title(schema)
      (schema["title"] || []).map { |t| t["plain_text"] || t.dig("text", "content") }.join.presence
    end
  end
end
