# frozen_string_literal: true

module Scout
  # Single source of truth for the tools Scout can call. Each tool declares its
  # JSON-Schema parameters once; that same schema is sent to the LLM as a native
  # tool definition AND used to validate the model's arguments before execution.
  # No more hand-written prose schemas drifting from the implementations.
  #
  # Autonomy levels:
  #   :read    — side-effect-free; executed in the agent loop, result fed back.
  #   :confirm — mutates data; NEVER executed from model output. Surfaced as a
  #              one-click button the user confirms (AgentToolsController), which
  #              calls back into `run`. This is the prompt-injection firewall:
  #              a jailbroken model can propose, but only a human click executes.
  class ToolRegistry
    Tool = Data.define(:name, :description, :parameters, :autonomy, :runner) do
      def read? = autonomy == :read
      def confirm? = autonomy == :confirm
    end

    DATE = { "type" => "string", "description" => "ISO date YYYY-MM-DD" }.freeze

    def self.tools
      @tools ||= build_tools.freeze
    end

    def self.find(name) = tools.find { |t| t.name == name.to_s }

    def self.read_tools = tools.select(&:read?)
    def self.confirm_tools = tools.select(&:confirm?)

    # Native tool payload for the adapters: [{name:, description:, parameters:}].
    def self.provider_payload(only: nil)
      scope = only ? tools.select { |t| Array(only).include?(t.name) } : tools
      scope.map { |t| { name: t.name, description: t.description, parameters: t.parameters } }
    end

    # Validate `args` against the tool's schema, then execute. Returns a plain
    # Hash safe to feed back to the model (or render). Never raises.
    def self.run(name, args)
      tool = find(name)
      return { error: "Unknown tool: #{name}" } unless tool

      args = (args || {}).deep_stringify_keys
      errors = validation_errors(tool, args)
      return { error: "Invalid arguments for #{name}: #{errors.join('; ')}" } if errors.any?

      tool.runner.call(args)
    rescue => e
      Rails.logger.error("[Scout::ToolRegistry] #{name} failed: #{e.class}: #{e.message}")
      { error: "#{name} failed: #{e.message}" }
    end

    def self.validation_errors(tool, args)
      JSONSchemer.schema(tool.parameters).validate(args).map { |e| e.fetch("error", e["type"].to_s) }
    rescue => e
      Rails.logger.warn("[Scout::ToolRegistry] schema validation skipped for #{tool.name}: #{e.message}")
      []
    end

    def self.object_schema(properties)
      { "type" => "object", "additionalProperties" => false, "properties" => properties }
    end

    def self.build_tools
      [
        Tool.new(
          name: "query_emails",
          description: "Search and filter the user's email messages. Use before stating any numbers about email.",
          autonomy: :read,
          parameters: object_schema(
            "status" => { "type" => "string", "enum" => %w[fetched processed ignored] },
            "ai_priority" => { "type" => "string", "enum" => %w[low medium high] },
            "tag_name" => { "type" => "string" },
            "contact_email" => { "type" => "string" },
            "has_attachment" => { "type" => "boolean" },
            "search_text" => { "type" => "string", "description" => "semantic/keyword query" },
            "date_from" => DATE, "date_to" => DATE,
            "limit" => { "type" => "integer", "minimum" => 1, "maximum" => 50 }
          ),
          runner: ->(args) { Tools::QueryEmails.call(args) }
        ),
        Tool.new(
          name: "query_documents",
          description: "Search and filter the user's documents (invoices, receipts, statements, etc.).",
          autonomy: :read,
          parameters: object_schema(
            "status" => { "type" => "string", "enum" => %w[pending processed review approved failed] },
            "document_type" => { "type" => "string" },
            "vendor_name" => { "type" => "string" },
            "amount_min_cents" => { "type" => "integer" },
            "amount_max_cents" => { "type" => "integer" },
            "source" => { "type" => "string", "enum" => %w[manual_upload email] },
            "search_text" => { "type" => "string" },
            "date_from" => DATE, "date_to" => DATE,
            "limit" => { "type" => "integer", "minimum" => 1, "maximum" => 50 }
          ),
          runner: ->(args) { Tools::QueryDocuments.call(args) }
        ),
        Tool.new(
          name: "query_contacts",
          description: "Search and filter the user's contacts.",
          autonomy: :read,
          parameters: object_schema(
            "name" => { "type" => "string" },
            "email" => { "type" => "string" },
            "organization" => { "type" => "string" },
            "relationship_type" => { "type" => "string" },
            "limit" => { "type" => "integer", "minimum" => 1, "maximum" => 50 }
          ),
          runner: ->(args) { Tools::QueryContacts.call(args) }
        ),
        Tool.new(
          name: "generate_report",
          description: "Aggregate statistics across email, documents, contacts, or tags.",
          autonomy: :read,
          parameters: object_schema(
            "type" => { "type" => "string", "enum" => %w[email_summary document_summary contact_summary tag_distribution] },
            "date_from" => DATE, "date_to" => DATE
          ),
          runner: ->(args) { Tools::GenerateReport.call(args) }
        ),
        Tool.new(
          name: "bulk_archive",
          description: "Archive multiple emails. Proposes the action for one-click user confirmation; do not claim it is done.",
          autonomy: :confirm,
          parameters: object_schema(
            "email_ids" => { "type" => "array", "items" => { "type" => "string" } },
            "status" => { "type" => "string" }, "tag_name" => { "type" => "string" },
            "date_from" => DATE, "date_to" => DATE
          ),
          runner: ->(args) { Tools::BulkArchive.call(args) }
        ),
        Tool.new(
          name: "bulk_tag",
          description: "Add or remove a tag on multiple emails. Proposes for one-click confirmation.",
          autonomy: :confirm,
          parameters: object_schema(
            "tag_name" => { "type" => "string" },
            "action" => { "type" => "string", "enum" => %w[add remove] },
            "email_ids" => { "type" => "array", "items" => { "type" => "string" } },
            "status" => { "type" => "string" }, "date_from" => DATE, "date_to" => DATE
          ),
          runner: ->(args) { Tools::BulkTag.call(args) }
        ),
        Tool.new(
          name: "reclassify",
          description: "Re-run AI classification on emails. Only when the user reports wrong categories. Proposes for confirmation.",
          autonomy: :confirm,
          parameters: object_schema(
            "email_ids" => { "type" => "array", "items" => { "type" => "string" } },
            "status" => { "type" => "string" }
          ),
          runner: ->(args) { Tools::Reclassify.call(args) }
        )
      ]
    end
  end
end
