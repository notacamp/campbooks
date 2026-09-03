# frozen_string_literal: true

# DEV-ONLY, opt-in browser aid for the "compose from intent" surface.
#
# The bold Desk drives the real compose-chat pipeline (EmailComposeChatController
# -> ComposeChatReplyJob -> Ai::ComposeChatService), which needs a running Solid
# Queue worker and a live AI provider. Neither exists in the local scratch dev
# box, so with COMPOSE_FAKE_REPLY=1 this file:
#
#   1. reports the text AI provider as available (so the guard passes and the
#      Shorter/Warmer footer shows),
#   2. runs ComposeChatReplyJob inline (no worker), and
#   3. returns a canned Scout reply + a canned body rewrite (no model call).
#
# It is a no-op in every other case (guarded on env + the flag), and is never
# loaded in test or production. Not part of the feature — a demo/verification
# convenience only.
if Rails.env.development? && ENV["COMPOSE_FAKE_REPLY"] == "1"
  Rails.application.config.after_initialize do
    # 1) Pretend a text provider is configured.
    ApplicationController.class_eval do
      def ai_provider_available?(_capability = :text) = true
    end

    # 2) Run the reply job inline so the auto-action broadcast lands without a worker.
    ComposeChatReplyJob.singleton_class.prepend(Module.new do
      def perform_later(*args) = perform_now(*args)
    end)

    # 3) Canned Scout reply: fill the editor from the intent note.
    Ai::ComposeChatService.prepend(Module.new do
      def reply_to(latest_message)
        note = latest_message.content.to_s.strip
        name = @user.name.presence || "there"
        body = "<p>Hi,</p><p>#{ERB::Util.html_escape(note)}</p>" \
               "<p>Happy to talk it through — just say the word.</p>" \
               "<p>Best,<br>#{ERB::Util.html_escape(name)}</p>"
        {
          reply: "Here is a first draft — edit it freely.",
          auto_actions: [ { "tool" => "set_body", "args" => { "body" => body } } ],
          suggested_actions: [],
          questions: []
        }
      end
    end)

    # 3b) Canned body rewrite for the Shorter / Warmer / Firmer footer buttons.
    Ai::DraftRewriter.prepend(Module.new do
      def rewrite(body_html, tone:, style: nil)
        return nil unless Ai::DraftRewriter::TONES.include?(tone.to_s)

        text = ActionView::Base.full_sanitizer.sanitize(body_html.to_s).squish
        return nil if text.blank?

        case tone.to_s
        when "shorter" then "<p>#{ERB::Util.html_escape(text.split(/(?<=[.!?])\s+/).first.to_s)}</p>"
        when "warmer"  then "<p>Hi there — hope you're well!</p><p>#{ERB::Util.html_escape(text)}</p>"
        else                "<p>To be direct: #{ERB::Util.html_escape(text)}</p>"
        end
      end
    end)

    Rails.logger.info("[compose_dev_stubs] COMPOSE_FAKE_REPLY active — Scout drafts + rewrites are canned.")
  end
end
