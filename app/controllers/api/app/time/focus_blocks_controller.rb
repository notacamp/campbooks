# frozen_string_literal: true

module Api
  module App
    module Time
      # Keep / Move / Dismiss a Scout focus block. Mirrors FocusBlocksController but
      # returns JSON. Each mutation returns the updated focus block so the SPA can
      # patch its cache in place.
      class FocusBlocksController < Api::App::BaseController
        before_action :set_focus_block

        # POST /api/app/focus_blocks/:id/keep
        def keep
          result = ::Time::FocusKeeper.call(@focus_block, user: current_user)
          if result.success?
            render_data(focus_block_response(@focus_block))
          else
            render_error("keep_failed", result.error, status: :unprocessable_entity)
          end
        end

        # PATCH /api/app/focus_blocks/:id/move
        def move
          slot = parse_slot(params[:start_at])
          return render_error("invalid_slot", "Invalid or past start_at.", status: :unprocessable_entity) unless slot

          @focus_block.update!(start_at: slot, end_at: slot + @focus_block.duration_minutes.minutes, status: :moved)
          render_data(focus_block_response(@focus_block))
        end

        # DELETE /api/app/focus_blocks/:id
        def dismiss
          @focus_block.dismissed!
          render_data(focus_block_response(@focus_block))
        end

        private

        def set_focus_block
          @focus_block = FocusBlock.accessible_to(current_user).find(params[:id])
        end

        def parse_slot(value)
          parsed = ::Time.zone.parse(value.to_s)
          parsed if parsed && parsed.future?
        rescue ArgumentError
          nil
        end

        def focus_block_response(block)
          {
            item: {
              id:                block.id,
              status:            block.status,
              title:             block.title,
              start_at:          block.start_at&.iso8601,
              end_at:            block.end_at&.iso8601,
              duration_minutes:  block.duration_minutes,
              task_id:           block.task_id,
              calendar_event_id: block.calendar_event_id
            }
          }
        end
      end
    end
  end
end
