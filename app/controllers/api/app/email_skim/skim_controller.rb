# frozen_string_literal: true

module Api
  module App
    module EmailSkim
      # Email Skim ring-deck for the SPA. Mirrors SkimController but returns JSON.
      # The deck is stateless (rebuilt from the user's inbox scope on each request).
      class SkimController < Api::App::BaseController
        # GET /api/app/email_skim
        def show
          rings = current_rings
          render_data(Api::App::EmailSkimDeckSerializer.new(rings).as_json)
        end

        # GET /api/app/email_skim/tray
        def tray
          render_data(Api::App::EmailSkimDeckSerializer.new(current_rings).as_json)
        end

        # POST /api/app/email_skim/decide
        # Archives the cluster's emails and records the decision.
        def decide
          archived = ::Emails::SkimArchive.new(current_user, params[:email_ids]).call
          record_decision("archive")
          render_data({ archived: archived })
        end

        # POST /api/app/email_skim/undo
        def undo
          restored = ::Emails::SkimRestore.new(current_user, params[:email_ids]).call
          render_data({ restored: restored })
        end

        # POST /api/app/email_skim/keep
        def keep
          kept = ::Emails::SkimDismiss.new(current_user, params[:email_ids]).call
          record_decision("keep")
          render_data({ kept: kept })
        end

        # POST /api/app/email_skim/promote
        def promote
          promoted = ::Emails::SkimPromote.new(current_user, params[:email_ids]).call
          record_decision("promote")
          render_data({ promoted: promoted })
        end

        # POST /api/app/email_skim/unpromote
        def unpromote
          unpromoted = ::Emails::SkimUnpromote.new(current_user, params[:email_ids]).call
          render_data({ unpromoted: unpromoted })
        end

        # POST /api/app/email_skim/dismiss_follow_up
        def dismiss_follow_up
          thread_ids = EmailMessage.where(email_account: current_user.readable_email_accounts,
                                          id: params[:email_ids])
                                   .distinct.pluck(:email_thread_id).compact
          dismissed = EmailThread.where(id: thread_ids)
                                 .update_all(follow_up_dismissed_at: ::Time.current)
          render_data({ dismissed: dismissed })
        end

        # POST /api/app/email_skim/sender_action
        def sender_action
          tool = params[:tool].to_s
          unless skim_sender_tool?(tool)
            return render_error("unknown_action", "Unknown sender action.", status: :unprocessable_entity)
          end

          email = EmailMessage.where(email_account: current_user.readable_email_accounts)
                              .where(id: params[:email_ids])
                              .order(received_at: :desc).first
          return render_error("not_found", "Email not found.", status: :not_found) unless email

          result = EmailActions.run(tool, email_message: email, user: current_user)
          render_data({ success: result[:success], message: result[:message] })
        end

        # GET /api/app/email_skim/emails/:id
        def email
          email = EmailMessage.where(email_account: current_user.readable_email_accounts)
                              .find(params[:id])
          render_data({
            id:           email.id,
            subject:      email.subject,
            from_address: email.from_address,
            from_name:    email.from_name,
            received_at:  email.received_at&.iso8601,
            body_html:    email.try(:body_html).presence || email.try(:body),
            can_reply:    email.email_account.sendable_by?(current_user)
          })
        end

        # POST /api/app/email_skim/emails/:id/reply
        def reply
          email = EmailMessage.where(email_account: current_user.readable_email_accounts)
                              .find(params[:id])

          unless email.email_account.sendable_by?(current_user)
            return render_error("forbidden", "Cannot send from this account.", status: :forbidden)
          end

          body = params[:body].to_s
          if body.strip.empty?
            return render_error("empty_body", "Reply body cannot be blank.", status: :unprocessable_entity)
          end

          client = email.email_account.mail_client
          draft  = client.save_draft(
            subject:    "Re: #{email.subject}",
            body:       body,
            to_address: ::Emails::ComposePrefill.reply_to_address(email)
          )
          draft_id = draft && (draft["messageId"] || draft["id"])
          return render_error("send_failed", "Failed to send reply.", status: :unprocessable_entity) unless draft_id

          client.send_draft(draft_id)
          email.email_thread&.update_column(:last_outbound_at, ::Time.current)
          render_data({ sent: true })
        rescue ActiveRecord::RecordNotFound
          raise
        rescue => e
          Rails.logger.error("[api/email_skim/reply] #{e.class}: #{e.message}")
          render_error("send_failed", "Failed to send reply.", status: :unprocessable_entity)
        end

        private

        def current_rings
          ::Emails::SkimDeck.for(
            current_user,
            now: ::Time.current,
            whitelist_mode: current_workspace&.whitelist_mode?,
            memory: ::Emails::SkimActionMemory.new(current_user)
          )
        end

        def record_decision(action)
          ::Emails::SkimDecisionRecorder.record(current_user, params[:email_ids], action: action)
        end

        def skim_sender_tool?(tool)
          defn = EmailActions.definition(tool)
          defn&.target == :sender && defn.surfaces.include?(:skim)
        end
      end
    end
  end
end
