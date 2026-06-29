class EmailMessageZohoLabelsController < ApplicationController
  before_action :require_authentication
  before_action :set_message

  def create
    # `.visible` guards against assigning a hidden system/low-value label (404 if
    # a stale UI or a crafted request sends one) — hidden labels are never chips.
    @tag = Current.workspace.tags.external.visible.find(params[:tag_id])

    begin
      label_assignment_service.new.apply(message: @message, tag: @tag)
    rescue Zoho::LabelAssignmentService::Error, Google::LabelAssignmentService::Error => e
      Rails.logger.error("[EmailMessageLabels] Apply failed: #{e.message}")
    end

    respond_to(&:turbo_stream)
  end

  def destroy
    @tag = @message.tags.external.find(params[:id])

    begin
      label_assignment_service.new.remove(message: @message, tag: @tag)
    rescue Zoho::LabelAssignmentService::Error, Google::LabelAssignmentService::Error => e
      Rails.logger.error("[EmailMessageLabels] Remove failed: #{e.message}")
    end

    respond_to(&:turbo_stream)
  end

  private

  def label_assignment_service
    @message.email_account.google? ? Google::LabelAssignmentService : Zoho::LabelAssignmentService
  end

  private

  def set_message
    @message = EmailMessage.where(email_account: Current.user.readable_email_accounts)
                           .find(params[:email_message_id])
  end
end
