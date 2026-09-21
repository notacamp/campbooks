# frozen_string_literal: true

module Api
  module App
    module Settings
      # Workspace invitation management. Sending is open to all members; cancelling
      # and resending are restricted to the sender or an admin. Approval is admin-only.
      class InvitationsController < Api::App::BaseController
        before_action :set_invitation, only: %i[destroy resend approve]
        before_action :require_invitation_manager, only: %i[destroy resend]

        # POST /api/app/settings/invitations
        def create
          invitation = current_workspace.invitations.new(invitation_params)
          invitation.invited_by = current_user

          unless Rails.application.config.self_hosted || current_user.admin?
            invitation.admin_approved = false
          end

          if invitation.save
            if invitation.admin_approved?
              InvitationMailer.invitation(invitation).deliver_later
            else
              Notifier.invitation_pending_approval(invitation)
            end
            render_data Api::App::Settings::InvitationSerializer.new(invitation).as_json,
                        status: :created
          else
            render_error("invalid", invitation.errors.full_messages.to_sentence,
                         status: :unprocessable_entity)
          end
        end

        # DELETE /api/app/settings/invitations/:id
        def destroy
          @invitation.cancel!
          Notifier.invitation_resolved(@invitation)
          render json: {}, status: :no_content
        end

        # POST /api/app/settings/invitations/:id/resend
        def resend
          @invitation.resend!
          if current_user.admin?
            @invitation.update!(admin_approved: true) unless @invitation.admin_approved?
            InvitationMailer.invitation(@invitation).deliver_later
            Notifier.invitation_resolved(@invitation)
          elsif @invitation.admin_approved?
            InvitationMailer.invitation(@invitation).deliver_later
          end
          render_data Api::App::Settings::InvitationSerializer.new(@invitation).as_json
        end

        # POST /api/app/settings/invitations/:id/approve
        def approve
          unless current_user.admin?
            return render_error("forbidden", "Admin access required.", status: :forbidden)
          end

          @invitation.approve_by_admin!
          Notifier.invitation_resolved(@invitation)
          render_data Api::App::Settings::InvitationSerializer.new(@invitation).as_json
        end

        private

        def set_invitation
          @invitation = current_workspace.invitations.find(params[:id])
        end

        def require_invitation_manager
          return if current_user.admin? || @invitation.invited_by_id == current_user.id

          render_error("forbidden", "You do not have permission to manage this invitation.",
                       status: :forbidden)
        end

        def invitation_params
          params.permit(:email)
        end
      end
    end
  end
end
