module Workflows
  # Trigger context for the `webhook` trigger. The inbound request's JSON body,
  # headers, and query string become Liquid variables (`payload`, `headers`,
  # `query`) so steps can route on and forward the external event.
  class WebhookContext < TriggerContext
    attr_reader :payload, :headers, :query, :source_ip

    def initialize(payload: {}, headers: {}, query: {}, source_ip: nil)
      @payload = payload || {}
      @headers = headers || {}
      @query = query || {}
      @source_ip = source_ip
    end

    def liquid_context
      # Top-level keys are always present (even when empty) so strict Liquid
      # rendering of `{{ payload.x }}` never blows up on an undefined variable.
      {
        "payload" => @payload,
        "headers" => @headers,
        "query" => @query
      }
    end

    def trigger_data
      {
        "type" => "webhook",
        "source_ip" => @source_ip,
        "payload" => @payload
      }
    end

    def step_input
      { "payload" => @payload }
    end
  end
end
