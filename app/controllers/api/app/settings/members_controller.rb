# frozen_string_literal: true

module Api
  module App
    module Settings
      # Workspace member list + role management. Role updates are admin-only.
      class MembersController < Api::App::BaseController
        # GET /api/app/settings/members
        def index
          members = current_workspace.users.order(:name)
          invitations = current_workspace.invitations.includes(:invited_by).order(created_at: :desc)

          render_data({
            members: members.map { |u| Api::App::Settings::MemberSerializer.new(u).as_json },
            invitations: invitations.map { |i| Api::App::Settings::InvitationSerializer.new(i).as_json }
          })
        end

        # PATCH /api/app/settings/members/:id
        def update
          unless current_user.admin?
            return render_error("forbidden", "Admin access required.", status: :forbidden)
          end

          member = current_workspace.users.find(params[:id])

          if member == current_user
            return render_error("invalid", "You cannot change your own role.", status: :unprocessable_entity)
          end

          new_role = params[:role].to_s
          unless User.roles.key?(new_role)
            return render_error("invalid", "Invalid role.", status: :unprocessable_entity)
          end

          member.update!(role: new_role)
          AuditEvent.log("workspace_role_changed", user: current_user, request: request, target: member, role: new_role)
          render_data Api::App::Settings::MemberSerializer.new(member).as_json
        end
      end
    end
  end
end
