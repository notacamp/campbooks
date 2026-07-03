module InboxSettings
  # Smart-groups panel: which low-priority buckets collapse into inbox group
  # rows. Server-side prefs (users.inbox_smart_groups) because they change the
  # inbox queries, unlike the localStorage-only display panel.
  class SmartGroupsController < BaseController
    def show
    end

    def update
      Current.user.update_smart_group_prefs!(smart_group_params)

      respond_to do |format|
        format.turbo_stream # -> update.turbo_stream.erb (re-render panel + toast)
        format.html { redirect_to inbox_settings_smart_groups_path }
      end
    end

    private

    def smart_group_params
      params.fetch(:smart_groups, {})
            .permit(:enabled, *User::SMART_GROUP_BUCKETS)
            .to_h
            .transform_values { |value| value == "1" }
    end
  end
end
