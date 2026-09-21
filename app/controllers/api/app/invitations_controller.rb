# frozen_string_literal: true

module Api
  module App
    # Invitation acceptance for the first-party app API.
    #
    # The web InvitationsController redirects unauthenticated users to
    # registration. Here we return a structured response so the SPA can route:
    #
    #   POST /api/app/invitations/:token/accept (unauthenticated OK)
    #     → { status: "registration_required", invitation_token, email, workspace_name }
    #       when the invited email has no account yet
    #     → { status: "login_required", email }
    #       when the invited email exists but no bearer was sent
    #     → { status: "already_member" }
    #       when the authenticated user is already in that workspace
    #     → { status: "accepted", workspace: { id, name } }
    #       when an authenticated user joins a new workspace
    class InvitationsController < BaseController
      # Accept must work before the user has a session (the invite link is the
      # first thing they open). skip_before_action affects ALL actions here.
      skip_before_action :authenticate_session_token!

      # POST /api/app/invitations/:token/accept
      def accept
        invitation = resolve_invitation
        return if performed?  # error already rendered

        email = invitation.email.downcase

        # ── Unauthenticated path ──────────────────────────────────────────────
        if Current.user.nil?
          user_exists = User.exists?(email_address: email)

          if user_exists
            return render_data({
              status: "login_required",
              email: invitation.email,
              workspace_name: invitation.workspace.name
            })
          else
            return render_data({
              status: "registration_required",
              invitation_token: invitation.token,
              email: invitation.email,
              workspace_name: invitation.workspace.name
            })
          end
        end

        # ── Authenticated path ────────────────────────────────────────────────
        if Current.user.workspace == invitation.workspace
          return render_data({ status: "already_member",
                               workspace_name: invitation.workspace.name })
        end

        invitation.accept!(Current.user)
        render_data({
          status: "accepted",
          workspace: {
            id: invitation.workspace.id,
            name: invitation.workspace.name
          }
        })
      rescue ActiveRecord::RecordInvalid => e
        render_error("invalid", e.record.errors.full_messages.to_sentence,
                     status: :unprocessable_entity)
      end

      private

      # Try to authenticate the session token if one was provided, but don't
      # require it (the action is reachable without auth). Override the base
      # before_action behaviour by doing authentication manually here.
      def resolve_invitation
        # Manually attempt bearer auth so authenticated users get the authed path.
        if (raw_token = bearer_token)
          if (session = Session.authenticate_api_token(raw_token))
            Current.session   = session
            Current.workspace = session.user.workspace
          end
        end

        invitation = Invitation.find_by!(token: params[:token])
        check_invitation_validity(invitation)
        invitation
      rescue ActiveRecord::RecordNotFound
        render_not_found
        nil
      end

      def check_invitation_validity(invitation)
        if invitation.accepted?
          render_error("already_accepted", "This invitation has already been accepted.",
                       status: :unprocessable_entity)
        elsif invitation.cancelled?
          render_error("cancelled", "This invitation has been cancelled.",
                       status: :unprocessable_entity)
        elsif invitation.expired?
          render_error("expired", "This invitation has expired.",
                       status: :unprocessable_entity)
        elsif !Rails.application.config.self_hosted && !invitation.admin_approved?
          render_error("pending_approval", "This invitation is awaiting admin approval.",
                       status: :forbidden)
        end
      end
    end
  end
end
