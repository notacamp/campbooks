module InboxSettings
  # Provider (Zoho/Gmail) labels for a single email account. These are Tag
  # records with source: :external. Create/update/destroy/sync all call the
  # provider's mail API, so they mutate the real mailbox.
  class ExternalLabelsController < BaseController
    before_action :set_accounts

    def index
      @labels = @account ? @account.external_tags.by_name : []
    end

    def create
      return respond_with_labels("Pick an account first.", severity: :warning) unless @account

      result = @account.mail_client.create_label(name: params[:name], color: params[:color])
      label_id = result && (result["labelId"] || result["id"])

      if label_id
        Tag.find_or_create_by!(email_account_id: @account.id, external_label_id: label_id) do |tag|
          tag.name = result["displayName"] || result["name"] || params[:name]
          tag.color = result["color"] || params[:color] || "#ffd700"
          tag.source = :external
        end
        respond_with_labels("Label created.")
      else
        respond_with_labels("Couldn't create the label.", severity: :error)
      end
    end

    def update
      tag = @account.external_tags.find(params[:id])
      result = @account.mail_client.update_label(tag.external_label_id, name: params[:name], color: params[:color])

      if result.dig("status", "code") == 200 || result["id"].present?
        tag.update!(name: params[:name], color: params[:color])
        respond_with_labels("Label updated.")
      else
        respond_with_labels("Couldn't update the label.", severity: :error)
      end
    end

    def destroy
      tag = @account.external_tags.find(params[:id])
      begin
        @account.mail_client.delete_label(tag.external_label_id)
      rescue => e
        Rails.logger.error("[InboxSettings::ExternalLabels] provider delete failed: #{e.message}")
      end
      tag.destroy!
      respond_with_labels("Label deleted.")
    end

    def sync
      count = label_sync_service.new(@account).sync_labels!
      respond_with_labels("Synced #{count} labels.")
    end

    private

    def respond_with_labels(message, severity: :success)
      @labels = @account ? @account.external_tags.by_name : []
      render :list_stream, locals: { message: message, severity: severity }
    end

    def label_sync_service
      @account.google? ? Google::LabelSyncService : Zoho::LabelSyncService
    end

    def set_accounts
      @accounts = Current.user.readable_email_accounts.to_a
      @account =
        if params[:email_account_id].present?
          @accounts.find { |a| a.id.to_s == params[:email_account_id].to_s }
        else
          @accounts.first
        end
    end
  end
end
