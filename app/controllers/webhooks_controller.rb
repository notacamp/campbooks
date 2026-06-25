# Public inbound-webhook endpoint. Any external service can POST (or GET) to a
# workflow's unique URL — /webhooks/:token — to trigger it. The token in the
# path is the shared secret, so this controller is unauthenticated but the work
# itself runs in a background job scoped to that one workflow.
class WebhooksController < ApplicationController
  allow_unauthenticated_access
  skip_before_action :ensure_workspace
  skip_before_action :redirect_to_onboarding_if_incomplete
  skip_forgery_protection

  # Throttle per token+IP to limit brute-force token enumeration and abuse.
  rate_limit to: 60, within: 1.minute, by: -> { "#{params[:token]}:#{request.remote_ip}" },
             with: -> { render json: { ok: false, error: "Too many requests" }, status: :too_many_requests }

  before_action :check_payload_size

  # Headers that may carry credentials — never persist these in execution data.
  SENSITIVE_HEADERS = %w[Cookie Authorization X-Api-Key X-Api-Token Proxy-Authorization].freeze

  def receive
    # Workflows ship gated off by default (Features.workflows?) — when off the
    # ingress is inert. Return the same 404 as an unknown token so a disabled
    # feature doesn't reveal itself.
    return render(json: { ok: false, error: "Unknown or inactive webhook" }, status: :not_found) unless Features.workflows?

    # Look up by token first for efficiency, then confirm with a constant-time
    # comparison (mirrors CalendarWebhooksController) to prevent timing oracles.
    candidate = Workflow.enabled.find_by(webhook_token: params[:token])
    token_valid = candidate && ActiveSupport::SecurityUtils.secure_compare(
      candidate.webhook_token.to_s, params[:token].to_s
    )
    workflow = token_valid ? candidate : nil

    unless workflow&.webhook?
      return render json: { ok: false, error: "Unknown or inactive webhook" }, status: :not_found
    end

    WorkflowWebhookJob.perform_later(
      workflow.id,
      payload: extract_payload,
      headers: safe_headers,
      query: request.query_parameters,
      source_ip: request.remote_ip
    )

    render json: { ok: true, workflow: workflow.name, message: "Workflow triggered" }, status: :accepted
  end

  private

  # Reject oversized bodies before any parsing or job enqueue.
  # 512 KB is generous for a webhook ping; larger payloads are almost certainly
  # an abuse attempt or a misconfigured client.
  def check_payload_size
    if request.content_length.to_i > 512_000
      render json: { ok: false, error: "Payload too large" }, status: :payload_too_large
    end
  end

  def extract_payload
    if request.content_type.to_s.include?("application/json")
      raw = request.raw_post
      raw.present? ? JSON.parse(raw) : {}
    else
      request.request_parameters.presence || request.query_parameters
    end
  rescue JSON::ParserError
    { "_raw" => request.raw_post.to_s.truncate(5000) }
  end

  def safe_headers
    request.headers.env.each_with_object({}) do |(key, value), acc|
      next unless key.start_with?("HTTP_")
      next unless value.is_a?(String)

      name = key.delete_prefix("HTTP_").split("_").map(&:capitalize).join("-")
      next if SENSITIVE_HEADERS.include?(name)

      acc[name] = value
    end
  end
end
