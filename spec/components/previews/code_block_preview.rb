# frozen_string_literal: true

class CodeBlockPreview < Lookbook::Preview
  # A captured webhook payload, pretty-printed.
  def json_payload
    render(Campbooks::CodeBlock.new(content: {
      "event" => "invoice.paid",
      "amount" => 4200,
      "customer" => { "name" => "Acme Corp", "email" => "billing@acme.test" }
    }))
  end

  # An HTTP response result.
  def http_result
    render(Campbooks::CodeBlock.new(content: {
      "request" => { "method" => "POST", "url" => "https://hooks.slack.com/services/XXX" },
      "response" => { "status" => 200, "body" => "ok" }
    }))
  end

  # Raw string content.
  def raw_string
    render(Campbooks::CodeBlock.new(content: "HTTP 500: Internal Server Error"))
  end

  # Empty content falls back to a placeholder.
  def empty
    render(Campbooks::CodeBlock.new(content: nil))
  end
end
