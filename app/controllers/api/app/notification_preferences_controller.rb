# frozen_string_literal: true

module Api
  module App
    # Notification preferences — per-category in-app + email toggles.
    class NotificationPreferencesController < Api::App::BaseController
      # GET /api/app/notification_preferences
      def index
        org = current_workspace
        tags = org.tags.order(:name).to_a
        doc_types = org.document_types.order(:name).to_a

        prefs_by_key = current_user.notification_preferences.index_by do |p|
          p.tag? ? "tag:#{p.tag_id}" : "document_type:#{p.document_type_id}"
        end

        render_data({
          digest_preference: current_user.email_on_waiting_on_replies_digest,
          tags: tags.map { |t| tag_pref(t, prefs_by_key["tag:#{t.id}"]) },
          document_types: doc_types.map { |dt| doc_type_pref(dt, prefs_by_key["document_type:#{dt.id}"]) }
        })
      end

      # PATCH /api/app/notification_preferences/toggle
      def toggle
        pref_params = params
        kind = pref_params[:kind].to_s.to_sym

        pref = case kind
        when :tag
                 tag_id = pref_params[:tag_id]
                 current_user.notification_preferences
                             .find_or_initialize_by(kind: :tag, tag_id: tag_id)
        when :document_type
                 doc_type_id = pref_params[:document_type_id]
                 current_user.notification_preferences
                             .find_or_initialize_by(kind: :document_type, document_type_id: doc_type_id)
        else
                 return render_error("invalid", "Invalid preference kind.", status: :unprocessable_entity)
        end

        if pref_params.key?(:notify_in_app)
          pref.notify_in_app = ActiveModel::Type::Boolean.new.cast(pref_params[:notify_in_app])
        end
        if pref_params.key?(:notify_email)
          pref.notify_email = ActiveModel::Type::Boolean.new.cast(pref_params[:notify_email])
        end
        if !pref_params.key?(:notify_in_app) && !pref_params.key?(:notify_email)
          pref.notify_in_app = !pref.notify_in_app?
        end

        pref.save!
        render_data preference_data(pref)
      end

      # PATCH /api/app/notification_preferences/bulk_toggle
      def bulk_toggle
        kind = params[:kind].to_s.to_sym
        channel = params[:channel].to_s.to_sym
        value = ActiveModel::Type::Boolean.new.cast(params[:value])
        column = channel == :in_app ? :notify_in_app : :notify_email

        targets = case kind
        when :tag
                    current_workspace.tags.pluck(:id)
        when :document_type
                    current_workspace.document_types.pluck(:id)
        else
                    return render_error("invalid", "Invalid preference kind.", status: :unprocessable_entity)
        end

        targets.each do |target_id|
          pref = case kind
          when :tag
                   current_user.notification_preferences.find_or_initialize_by(kind: :tag, tag_id: target_id)
          when :document_type
                   current_user.notification_preferences.find_or_initialize_by(kind: :document_type, document_type_id: target_id)
          end
          pref.update!(column => value)
        end

        render json: { data: { updated: true } }
      end

      # PATCH /api/app/settings/notifications/digest_preference
      def digest_preference
        current_user.update(params.permit(:email_on_waiting_on_replies_digest))
        render_data({ digest_preference: current_user.email_on_waiting_on_replies_digest })
      end

      private

      def tag_pref(tag, pref)
        {
          kind: "tag",
          tag_id: tag.id,
          tag_name: tag.name,
          notify_in_app: pref&.notify_in_app != false,
          notify_email: pref&.notify_email != false
        }
      end

      def doc_type_pref(doc_type, pref)
        {
          kind: "document_type",
          document_type_id: doc_type.id,
          document_type_name: doc_type.name,
          notify_in_app: pref&.notify_in_app != false,
          notify_email: pref&.notify_email != false
        }
      end

      def preference_data(pref)
        {
          kind: pref.kind,
          tag_id: pref.tag_id,
          document_type_id: pref.document_type_id,
          notify_in_app: pref.notify_in_app,
          notify_email: pref.notify_email
        }
      end
    end
  end
end
