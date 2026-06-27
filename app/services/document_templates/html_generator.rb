module DocumentTemplates
  class HtmlGenerator
    Result = Data.define(:ok, :html_content, :variables_schema, :ai_provenance, :error)
    def self.call(user_description:, workspace:) = new(user_description, workspace).call
    def call
      config = Ai::Configuration.for(:document_template_generation)
      return Result.new(ok: false, error: "AI not configured") unless config
      response = config[:adapter].chat(system: system_prompt(config), messages: [{role:"user",content:user_message}], model:config[:model], max_tokens:config[:max_tokens]||4000, temperature:config[:temperature]||0.3)
      parsed = Ai::ChatService.parse_json_response(response, object_start: /\{\s*"html_content"/)
      Result.new(ok:true, html_content:parsed["html_content"], variables_schema:parsed["variables_schema"]||[], ai_provenance:Ai::Provenance.from_config(config), error:nil)
    rescue => e
      Rails.logger.warn("[DocumentTemplates::HtmlGenerator] #{e.message}")
      Result.new(ok:false, html_content:nil, variables_schema:nil, ai_provenance:{}, error:e.message)
    end
    private
    def initialize(d,w)=(@user_description=d;@workspace=w)
    def user_message="Generate HTML document template:\n\n#{@user_description}\n\nRespond JSON only: {\"html_content\":\"...\",\"variables_schema\":[...]}"
    def system_prompt(config)
      base=Ai::ChatService.base_prompt(:document_template_generation)
      custom=config[:system_prompt]
      "#{base}\nGenerate HTML5 with embedded CSS for A4 print. Use {{var}} Liquid syntax. JSON only.#{custom ? "\n\n#{custom}" : ""}"
    end
  end
end
