# frozen_string_literal: true

# DEV/TEST ONLY — a canned Scout reply for exercising the Scout overlay's
# "Ask Scout" flow in a browser WITHOUT calling a real AI provider or running a
# Solid Queue worker. Inert unless SCOUT_FAKE_REPLY=1 is set in development or
# test; it is never active in production.
#
# It does three things while the flag is set:
#   1. Reports the :text capability as available, so the overlay offers the
#      "Ask Scout" row (the dev database has no provider configured).
#   2. Replaces AgentChatReplyJob#compute_reply with a fixed reply (text +
#      tool-step trace + follow-up prompts + one suggested action) — no provider
#      call, no network.
#   3. Runs jobs with the in-process :async adapter so the reply streams back
#      without a separate worker.
#
# The suggested action uses empty email_ids (the canned reply can't know real
# record ids), so clicking it posts through scout/tool and archives nothing —
# enough to verify the chip posts.
if ENV["SCOUT_FAKE_REPLY"] == "1" && (Rails.env.development? || Rails.env.test?)
  Rails.application.config.after_initialize do
    Ai::ProviderSetup.singleton_class.prepend(Module.new do
      def available?(workspace, capability)
        return true if capability.to_sym == :text

        super
      end
    end)

    AgentChatReplyJob.prepend(Module.new do
      private

      def compute_reply(_thread, message, on_status)
        on_status.call("Thinking…") if on_status
        {
          reply: "Here's what I found — a canned reply (SCOUT_FAKE_REPLY); no AI " \
                 "provider was called.\n\nYou asked: \"#{message.content}\". I looked " \
                 "across your inbox and documents.",
          thinking: nil,
          steps: [
            { "tool" => "query_emails", "args" => { "limit" => 5 }, "result" => { "count" => 3, "messages" => "3 items" } },
            { "tool" => "query_documents", "args" => {}, "result" => { "count" => 2, "documents" => "2 items" } }
          ],
          suggested_actions: [ { "tool" => "bulk_archive", "args" => { "email_ids" => [] }, "label" => "Archive these" } ],
          prompts: [ "What needs me today?", "Summarize my unread email", "Show unpaid invoices" ],
          provenance: {},
          auto_actions: []
        }
      end
    end)

    ActiveJob::Base.queue_adapter = :async
  end
end
