# frozen_string_literal: true

module Api
  module App
    module Now
      # POST /api/app/now/log/:id/undo
      # Undo one of Scout's logged actions from the Now deck log.
      # Reversible: email.archived → unarchive, email.tagged → remove_tag.
      class LogController < Api::App::BaseController
        def undo
          event = accessible_system_event(params[:id])
          return render_not_found unless event

          result = reverse_event(event)

          if result[:success]
            render_data({
              event_id: event.id,
              undone:   true,
              message:  result[:message]
            })
          else
            render_error("action_failed", result[:message] || "Cannot undo this action.",
                         status: :unprocessable_entity)
          end
        end

        private

        def accessible_system_event(id)
          return nil unless current_workspace

          current_workspace.events
                           .accessible_to(current_user)
                           .where(actor_id: nil)
                           .find_by(id: id)
        end

        def reverse_event(event)
          case event.name
          when "email.archived"
            return not_reversible unless event.subject.is_a?(EmailMessage)

            EmailActions.run("unarchive", email_message: event.subject, args: {}, user: current_user)
          when "email.tagged"
            tag_name = event.payload["tag"]
            return not_reversible unless event.subject.is_a?(EmailMessage) && tag_name.present?

            EmailActions.run("remove_tag", email_message: event.subject,
                             args: { tag_name: tag_name }, user: current_user)
          else
            not_reversible
          end
        end

        def not_reversible
          { success: false, message: "This action cannot be undone." }
        end
      end
    end
  end
end
