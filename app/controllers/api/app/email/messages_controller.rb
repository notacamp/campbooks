# frozen_string_literal: true

module Api
  module App
    module Email
      # Email message reads for the /api/app surface. Scoped to
      # EmailMessage.accessible_to(Current.user) so a bearer token only ever
      # sees mail from accounts it can read. The show endpoint marks the thread
      # read via Emails::MarkThreadRead (same path the web inbox uses) and
      # returns the full thread chain so the SPA can render folded older messages.
      class MessagesController < BaseController
        before_action :set_message, only: %i[show dismiss_todo dismiss_follow_up]

        def index
          scope = ::EmailMessage
                    .accessible_to(current_user)
                    .includes(:tags)
                    .order(received_at: :desc)
          scope = apply_filters(scope)
          @pagy, messages = pagy(scope, limit: per_page)
          render_page(messages.map { |m| MessageSerializer.new(m).as_json }, @pagy)
        end

        def show
          thread = @message.email_thread
          ::Emails::MarkThreadRead.call(thread) if thread
          thread_messages = thread ? thread.email_messages.includes(:tags).sort_by { |m| m.received_at || ::Time.at(0) } : []
          render_data(MessageSerializer.new(@message, detail: true, thread_messages: thread_messages).as_json)
        end

        def search
          scope = ::EmailMessage
                    .accessible_to(current_user)
                    .includes(:tags)
                    .order(received_at: :desc)
          if params[:q].present?
            like = "%#{params[:q]}%"
            scope = scope.where("email_messages.subject ILIKE :q OR email_messages.from_address ILIKE :q", q: like)
          end
          @pagy, messages = pagy(scope, limit: per_page)
          render_page(messages.map { |m| MessageSerializer.new(m).as_json }, @pagy)
        end

        def dismiss_todo
          @message.update!(ai_todo_dismissed: true)
          render_data(MessageSerializer.new(@message).as_json)
        end

        def dismiss_follow_up
          @message.email_thread&.update!(follow_up_dismissed_at: ::Time.current)
          render_data(MessageSerializer.new(@message).as_json)
        end

        private

        def set_message
          @message = ::EmailMessage.accessible_to(current_user).find(params[:id])
        end

        def apply_filters(scope)
          if params[:account_id].present?
            scope = scope.where(email_account_id: params[:account_id])
          end

          if params[:folder_id].present? && params[:folder_id] != "all"
            scope = scope.where(provider_folder_id: params[:folder_id])
          end

          unless params[:unread].nil?
            unread = ::ActiveModel::Type::Boolean.new.cast(params[:unread])
            scope = scope.where(read: !unread) unless unread.nil?
          end

          scope
        end
      end
    end
  end
end
