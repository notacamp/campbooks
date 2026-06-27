module DocumentTemplates
  # Generates an HTML document template (plus its variable schema) from a natural
  # language description, using the workspace's configured AI provider. Returns a
  # Result and never raises, so DocumentTemplateGenerationJob can simply branch on
  # `result.ok`.
  class HtmlGenerator
    Result = Data.define(:ok, :html_content, :variables_schema, :ai_provenance, :error) do
      def self.success(html_content:, variables_schema:, ai_provenance:)
        new(ok: true, html_content: html_content, variables_schema: variables_schema,
            ai_provenance: ai_provenance, error: nil)
      end

      def self.failure(error)
        new(ok: false, html_content: nil, variables_schema: nil, ai_provenance: {}, error: error)
      end
    end

    def self.call(user_description:, workspace:)
      new(user_description, workspace).call
    end

    def initialize(user_description, workspace)
      @user_description = user_description
      @workspace = workspace
    end

    def call
      config = Ai::Configuration.for(:document_template_generation)
      return Result.failure("AI is not configured for this workspace") unless config

      response = config[:adapter].chat(
        system: system_prompt(config),
        messages: [ { role: "user", content: user_message } ],
        model: config[:model],
        max_tokens: config[:max_tokens] || 4000,
        temperature: config[:temperature] || 0.3
      )

      parsed = Ai::ChatService.parse_json_response(response, object_start: /\{\s*"html_content"/)
      html = parsed["html_content"].to_s
      return Result.failure("The AI returned no HTML content") if html.blank?

      Result.success(
        html_content: html,
        variables_schema: normalize_schema(parsed["variables_schema"]),
        ai_provenance: Ai::Provenance.from_config(config)
      )
    rescue StandardError => e
      Rails.logger.warn("[DocumentTemplates::HtmlGenerator] #{e.class}: #{e.message}")
      Result.failure(e.message)
    end

    private

    attr_reader :user_description

    def user_message
      <<~MSG
        Generate an HTML document template based on this description:

        #{user_description}

        Respond with ONLY a JSON object (no markdown):
        { "html_content": "...", "variables_schema": [...] }
      MSG
    end

    def system_prompt(config)
      base = Ai::ChatService.base_prompt(:document_template_generation)
      custom = config[:system_prompt]

      <<~PROMPT.strip
        #{base}

        Generate an HTML document template for Campbooks.
        Create a complete HTML5 document with embedded CSS suitable for A4 print.
        Use {{ variable_name }} Liquid syntax for every piece of dynamic content,
        and describe each variable in variables_schema with the keys:
        key, label, type (text|date|number|email), required (boolean) and default.
        Respond with valid JSON only.
        #{custom.present? ? "\nWorkspace instructions:\n#{custom}" : ''}
      PROMPT
    end

    # The schema must be an array of hashes; coerce anything else to [] so a
    # surprising AI response can't corrupt the stored column.
    def normalize_schema(schema)
      return [] unless schema.is_a?(Array)

      schema.select { |entry| entry.is_a?(Hash) }
    end
  end
end
