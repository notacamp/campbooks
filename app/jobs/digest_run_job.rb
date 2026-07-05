# frozen_string_literal: true

# Materializes one DigestIssue for a given scheduled occurrence. Retries on
# transient errors; idempotency in Digests::Generator makes retries safe.
#
# `manual: true` bypasses the enabled check so a "Run now" action can trigger
# a digest even when it's paused.
class DigestRunJob < ApplicationJob
  queue_as :default
  retry_on StandardError, wait: :polynomially_longer, attempts: 3

  def perform(digest_id, period_end_iso, manual: false)
    return unless Features.digests?

    digest = ScheduledDigest.find_by(id: digest_id)
    return unless digest
    return unless digest.enabled || manual

    return unless digest.workspace.entitlements.feature?(:digests)

    # Mirror AgentChatReplyJob: set Current so Ai::Configuration and ai_prompts
    # read the correct workspace and user context inside the generator.
    Current.acting_user = digest.user
    Current.workspace   = digest.workspace

    Digests::Generator.new(digest).generate!(period_end: Time.iso8601(period_end_iso))
  ensure
    Current.acting_user = nil
    Current.workspace   = nil
  end
end
