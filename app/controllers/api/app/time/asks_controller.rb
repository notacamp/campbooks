# frozen_string_literal: true

module Api
  module App
    module Time
      # Mutations on asks (Tasks) from the Time surface: hold, schedule, snooze, done,
      # dismiss, hand_off, take_back. Every mutation returns the updated task plus an
      # undo_token for optimistic-update reversal. Reuses the exact same service layer
      # as AsksController (the web equivalent).
      #
      # 404-not-403 per the app convention — a cross-workspace ask looks like it doesn't exist.
      class AsksController < Api::App::BaseController
        before_action :require_tasks_enabled
        before_action :set_ask

        # POST /api/app/asks/:id/hold
        # Stakes a FocusBlock in Scout's earliest free slot via Time::FocusHolder.
        def hold
          result = ::Time::FocusHolder.call(@ask, user: current_user)
          if result.success?
            render_data(ask_response(@ask, undo_token: nil))
          else
            render_error("hold_failed", result.error, status: :unprocessable_entity)
          end
        end

        # PATCH /api/app/asks/:id/schedule
        # Sets due_on to a preset or ISO date in the user's zone.
        def schedule
          date = resolve_date(params[:on])
          return render_error("invalid_date", "Invalid or missing date.", status: :unprocessable_entity) unless date

          @ask.schedule!(date, zone: current_user.effective_time_zone, by: current_user)
          render_data(ask_response(@ask, undo_token: nil))
        end

        # POST /api/app/asks/:id/snooze
        def snooze
          @ask.snooze!(by: current_user)
          render_data(ask_response(@ask, undo_token: nil))
        end

        # POST /api/app/asks/:id/done
        def done
          @ask.move_to_status!(:done, by: current_user)
          render_data(ask_response(@ask, undo_token: nil))
        end

        # POST /api/app/asks/:id/dismiss
        def dismiss
          @ask.move_to_status!(:cancelled, by: current_user)
          render_data(ask_response(@ask, undo_token: nil))
        end

        # POST /api/app/asks/:id/hand_off
        # Reassigns the ask to a teammate. Target is params[:user_id].
        def hand_off
          target = handoff_target
          return render_not_found unless target

          Asks::HandOff.call(@ask, to: target, by: current_user)
          render_data(ask_response(@ask, undo_token: nil))
        end

        # POST /api/app/asks/:id/take_back
        # Reclaims a handed ask — only the assigner or an admin.
        def take_back
          return render_not_found unless can_take_back?

          @ask.task_assignments.destroy_all
          Events.publish("task.taken_back", subject: @ask, actor: current_user, payload: { title: @ask.title })
          Notification.where(notifiable: @ask).active.find_each(&:resolve!)
          render_data(ask_response(@ask, undo_token: nil))
        end

        private

        def set_ask
          @ask = Task.accessible_to(current_user).find(params[:id])
        end

        def require_tasks_enabled
          render_not_found unless Features.tasks?
        end

        def handoff_target
          current_workspace.users.where.not(id: current_user.id).find_by(id: params[:user_id])
        end

        def can_take_back?
          @ask.handed_by == current_user || current_user.admin?
        end

        def resolve_date(param)
          Asks::PresetDate.resolve(param, current_user.effective_time_zone)
        end

        def ask_response(ask, undo_token:)
          {
            item:       Api::V1::TaskSerializer.new(ask, detail: true).as_json,
            undo_token: undo_token
          }
        end
      end
    end
  end
end
