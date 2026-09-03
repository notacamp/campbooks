# frozen_string_literal: true

module Api
  module V1
    # Bulk email/thread actions over a selection of message ids — archive, mark
    # read/unread, move, tag, delete, snooze — dispatched through the same
    # Emails::BulkActions engine the web bulk toolbar uses. One expansion +
    # dispatch + broadcast path for both surfaces.
    #
    #   POST /api/v1/emails/bulk/:name
    #   body: { email_ids: [1,2,3], groups: ["Promos"], <tool options> }
    #
    # As with the single-message actions endpoint, the action name is the URL
    # segment (:name), not a body param called :action.
    class EmailBulkController < BaseController
      # API-exposed bulk tools → the OAuth scope each requires. The UI-only tools
      # (forward composer, process_ai, scout_chat) are deliberately not exposed.
      ACTION_SCOPES = {
        "archive"        => :"emails:write",
        "unarchive"      => :"emails:write",
        "mark_read"      => :"emails:write",
        "mark_unread"    => :"emails:write",
        "move_to_folder" => :"emails:write",
        "delete"         => :"emails:write",
        "snooze"         => :"emails:write",
        "unsnooze"       => :"emails:write",
        "tag"            => :"tags:write"
      }.freeze

      def create
        name = params[:name].to_s
        required_scope = ACTION_SCOPES[name]

        unless required_scope
          return render_api_error(
            "invalid_action",
            "Unknown bulk action '#{name}'. Supported actions: #{ACTION_SCOPES.keys.join(', ')}.",
            status: :unprocessable_entity
          )
        end

        doorkeeper_authorize!(required_scope)

        outcome = Emails::BulkActions.call(
          tool: name,
          user: Current.user,
          email_ids: params[:email_ids],
          groups: params[:groups],
          options: bulk_options
        )

        if outcome.empty_selection?
          return render_api_error("no_emails_selected",
                                  "No emails matched the selection.",
                                  status: :unprocessable_entity)
        end

        # A soft error from a tool (e.g. tag with no tag_name) surfaces as a
        # message rather than a nil result.
        if (message = outcome.error_message)
          return render_api_error("action_failed", message, status: :unprocessable_entity)
        end

        unless outcome.ok?
          return render_api_error("action_failed",
                                  "The bulk action could not be completed.",
                                  status: :unprocessable_entity)
        end

        render json: {
          data: { action: name, ids: outcome.selected_ids, result: outcome.result }
        }
      end

      private

      def bulk_options
        {
          folder_name: params[:folder_name],
          folder_id: params[:folder_id],
          tag_name: params[:tag_name],
          tag_action: params[:tag_action],
          snoozed_until: params[:snoozed_until]
        }
      end
    end
  end
end
