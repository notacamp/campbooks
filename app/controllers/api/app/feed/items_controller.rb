# frozen_string_literal: true

module Api
  module App
    module Feed
      # Feed item actions for the Now deck.
      #
      # POST /api/app/feed/items/:id/act      — perform the card's action
      # POST /api/app/feed/items/:id/dismiss  — dismiss without acting
      # POST /api/app/feed/items/:id/seen     — mark seen
      # POST /api/app/feed/items/:id/undo     — undo the last act
      # GET  /api/app/feed/items/:id/preview  — email preview for a card
      class ItemsController < Api::App::BaseController
        REVERSIBLE = %w[archive add_tag hold_task schedule_task snooze_task].freeze

        before_action :set_item

        # POST /api/app/feed/items/:id/act
        def act
          subject = @item.subject
          return render_error("gone", "Item subject no longer exists.", status: :unprocessable_entity) if subject.nil?

          result = perform_action(subject)

          if result[:success]
            @item.mark_acted!
            ::People::StandingsRefreshJob.enqueue_for(current_user.id)
            render_data({
              item:       Api::App::FeedItemSerializer.new(@item.reload, subject: @item.subject).as_json,
              message:    result[:message],
              reversible: REVERSIBLE.include?(params[:tool].to_s),
              undo_args:  result[:undo_args]
            })
          else
            render_error("action_failed", result[:message] || "Action failed.",
                         status: :unprocessable_entity)
          end
        end

        # POST /api/app/feed/items/:id/dismiss
        def dismiss
          @item.dismiss!
          ::People::StandingsRefreshJob.enqueue_for(current_user.id)
          render_data({
            item:    Api::App::FeedItemSerializer.new(@item).as_json,
            message: "Dismissed."
          })
        end

        # POST /api/app/feed/items/:id/seen
        def seen
          @item.mark_seen!
          head :no_content
        end

        # POST /api/app/feed/items/:id/undo
        def undo
          subject = @item.subject
          reverse_action(subject) if subject
          @item.reactivate!
          ::People::StandingsRefreshJob.enqueue_for(current_user.id)

          render_data({
            item:    Api::App::FeedItemSerializer.new(@item.reload, subject: @item.subject).as_json,
            message: "Restored."
          })
        end

        # GET /api/app/feed/items/:id/preview
        def preview
          msg = preview_message
          return render_error("no_preview", "No preview available.", status: :not_found) unless msg

          render_data(Api::App::MessageSerializer.new(msg).as_json)
        end

        private

        def set_item
          @item = current_user.feed_items.find(params[:id])
        rescue ActiveRecord::RecordNotFound
          render_not_found
        end

        def perform_action(subject)
          case @item.subject_type
          when "EmailMessage" then run_email_action(subject)
          when "Reminder"     then run_reminder_action(subject)
          when "Task"         then run_task_action(subject)
          when "Document"     then run_document_action(subject)
          else
            { success: false, message: "Unsupported item type." }
          end
        end

        def run_email_action(email)
          EmailActions.run(params[:tool], email_message: email,
                           args: params[:args] || {}, user: current_user)
        end

        def run_reminder_action(reminder)
          return { success: false, message: "Reminder not accessible." } unless reminder.workspace_id == current_user.workspace_id

          case params[:tool].to_s
          when "confirm"
            result = Reminders::Confirm.call(reminder, user: current_user)
            return { success: false, message: result.error } unless result.success?

            { success: true, message: "Reminder confirmed." }
          when "dismiss_reminder"
            reminder.dismissed!
            { success: true, message: "Reminder dismissed." }
          else
            { success: false, message: "Unsupported reminder action." }
          end
        end

        def run_task_action(task)
          return { success: false, message: "Ask not accessible." } unless task.workspace_id == current_user.workspace_id

          case params[:tool].to_s
          when "complete"
            task.move_to_status!(:done, by: current_user)
            { success: true, message: "Ask completed." }
          when "accept"
            task.move_to_status!(:todo, by: current_user)
            { success: true, message: "Ask accepted." }
          when "dismiss_task"
            task.move_to_status!(:cancelled, by: current_user)
            { success: true, message: "Ask dismissed." }
          else
            { success: false, message: "Unsupported ask action." }
          end
        end

        def run_document_action(document)
          return { success: false, message: "Document not accessible." } unless document.workspace_id == current_user.workspace_id

          case params[:tool].to_s
          when "mark_paid"
            document.mark_settled!
            { success: true, message: "Marked paid." }
          else
            { success: false, message: "Unsupported document action." }
          end
        end

        def reverse_action(subject)
          return unless subject

          case params[:tool].to_s
          when "archive"
            Tools::Unarchive.call(subject) if subject.is_a?(EmailMessage)
          when "add_tag"
            EmailActions.run("remove_tag", email_message: subject, args: params[:args] || {}, user: current_user) if subject.is_a?(EmailMessage)
          end
        rescue StandardError => e
          Rails.logger.error("[Api::App::Feed::ItemsController] reverse_action failed: #{e.class}: #{e.message}")
        end

        def preview_message
          id =
            if @item.kind == "follow_up" && @item.data["sent_message_id"].present?
              @item.data["sent_message_id"]
            else
              subject = @item.subject
              case subject
              when EmailMessage then subject.id
              when Reminder, Task then subject.source_email&.id
              end
            end

          id && EmailMessage.accessible_to(current_user)
                            .includes(:contact, :email_account, files_attachments: :blob)
                            .find_by(id: id)
        end
      end
    end
  end
end
