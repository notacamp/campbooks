# frozen_string_literal: true

module Api
  module App
    module Compose
      # Compose surface: new message, reply, AI tone rewrite, and intent prefill.
      # Sending reuses Emails::Sender (shared with the web compose and v1 API) so
      # the auth/account/threading/attachment logic lives in one place. The
      # rewrite endpoint delegates to Ai::DraftRewriter. Prefill delegates to
      # Emails::IntentPrefill and returns inferred To + Subject with
      # to_inferred/subject_inferred flags so the SPA can render "· inferred"
      # chips until the user edits.
      class MessagesController < BaseController
        # POST /api/app/compose/send
        def send_new
          args = build_send_args
          result = ::Emails::Sender.call(**args)
          render_send_result(result)
        end

        # POST /api/app/compose/reply
        def reply
          source_message = ::EmailMessage.accessible_to(current_user).find(params.require(:email_message_id))
          to = params[:to_address].presence || ::Emails::ComposePrefill.reply_to_address(source_message)
          subject = params[:subject].presence || "Re: #{source_message.subject}"

          result = ::Emails::Sender.call(
            user: current_user,
            source_message: source_message,
            email_account_id: params[:email_account_id],
            to_address: to,
            subject: subject,
            body: params.require(:body),
            cc_address: params[:cc_address],
            bcc_address: params[:bcc_address],
            attachment_signed_ids: Array(params[:attachment_signed_ids])
          )
          render_send_result(result)
        end

        # POST /api/app/compose/rewrite
        # body: { body_html:, tone: } — tone is one of shorter/warmer/firmer
        def rewrite
          body_html = params.require(:body_html)
          tone = params.require(:tone)
          rewritten = ::Ai::DraftRewriter.new.rewrite(
            body_html,
            tone: tone,
            style: current_user.writing_style_prompt
          )
          if rewritten
            render_data(Api::App::Compose::RewriteSerializer.new(rewritten, tone: tone).as_json)
          else
            render_error("rewrite_failed", "AI rewrite is currently unavailable.",
                         status: :unprocessable_entity)
          end
        end

        # GET /api/app/compose/prefill?intent=...&to=...
        def compose_prefill
          result = ::Emails::IntentPrefill.for(
            user: current_user,
            intent: params[:intent],
            to: params[:to]
          )
          render_data(Api::App::Compose::PrefillSerializer.new(result).as_json)
        end

        private

        def build_send_args
          body = params[:body].to_s
          if params[:signature_id].present?
            sig = current_user.signatures.find_by(id: params[:signature_id])
            body = ::Signature.append_to_body(body, sig) if sig
          end
          {
            user: current_user,
            email_account_id: params[:email_account_id],
            to_address: params.require(:to_address),
            subject: params[:subject],
            body: body,
            cc_address: params[:cc_address],
            bcc_address: params[:bcc_address],
            attachment_signed_ids: Array(params[:attachment_signed_ids])
          }
        end

        def render_send_result(result)
          if result.ok?
            render_data(
              { id: result.email_message&.id, provider_message_id: result.provider_message_id },
              status: :created
            )
          else
            status = result.error_code == "no_sendable_account" ? :forbidden : :unprocessable_entity
            render_error(result.error_code, result.error_message, status: status)
          end
        end
      end
    end
  end
end
