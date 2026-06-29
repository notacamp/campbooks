# frozen_string_literal: true

module EmailTemplates
  # Generates an email template (subject + inline-styled HTML body + variable
  # schema) from a plain-language description, via the workspace's configured AI.
  # Mirrors DocumentTemplates::HtmlGenerator but targets an email body (no A4/PDF
  # page CSS) and also produces a subject line.
  class HtmlGenerator
    Result = Data.define(:ok, :subject, :body_html, :variables_schema, :ai_provenance, :error)

    def self.call(user_description:, workspace:)
      new(user_description, workspace).call
    end

    def call
      config = Ai::Configuration.for(:email_template_generation)
      return Result.new(ok: false, subject: nil, body_html: nil, variables_schema: nil, ai_provenance: {}, error: "AI not configured") unless config

      response = config[:adapter].chat(
        system: system_prompt(config),
        messages: [ { role: "user", content: user_message } ],
        model: config[:model],
        max_tokens: config[:max_tokens] || 4000,
        temperature: config[:temperature] || 0.3
      )

      parsed = Ai::ChatService.parse_json_response(response, object_start: /\{\s*"subject"/)

      Result.new(
        ok: true,
        subject: parsed["subject"].to_s,
        body_html: parsed["body_html"].to_s,
        variables_schema: parsed["variables_schema"] || [],
        ai_provenance: Ai::Provenance.from_config(config),
        error: nil
      )
    rescue => e
      Rails.logger.warn("[EmailTemplates::HtmlGenerator] #{e.message}")
      Result.new(ok: false, subject: nil, body_html: nil, variables_schema: nil, ai_provenance: {}, error: e.message)
    end

    private

    def initialize(user_description, workspace)
      @user_description = user_description
      @workspace = workspace
    end

    def user_message
      <<~MSG
        Generate an email template based on this description:

        #{@user_description}

        Respond with ONLY a JSON object (no markdown):
        { "subject": "...", "body_html": "...", "variables_schema": [...] }
      MSG
    end

    def system_prompt(config)
      base = Ai::ChatService.base_prompt(:email_template_generation)
      custom = config[:system_prompt]
      <<~PROMPT
        #{base}

        Generate a reusable email template for Campbooks.
        Produce a subject line and an HTML email body. Use simple, email-client-safe
        HTML with inline styles only (no <head>, <style> blocks, scripts, or external CSS).
        Use {{ variable_name }} Liquid syntax for any dynamic content, and list each
        variable in variables_schema as { "key", "label", "type", "required", "default" }
        where type is one of text, string, date, number, email.
        Respond with valid JSON only.
        #{custom ? "\n\nWorkspace instructions:\n#{custom}" : ""}
      PROMPT
    end
  end
end
