class EmailMessageTagsController < ApplicationController
  before_action :require_authentication
  before_action :set_message

  def create
    # `.visible` guards against assigning a hidden system/low-value tag (404 if
    # a stale UI or a crafted request sends one) — hidden tags are never chips.
    @tag = Current.workspace.tags.visible.find(params[:tag_id])

    if @tag.external?
      begin
        label_assignment_service.new.apply(message: @message, tag: @tag)
      rescue Zoho::LabelAssignmentService::Error, Google::LabelAssignmentService::Error => e
        Rails.logger.error("[EmailMessageTags] Apply failed: #{e.message}")
      end
    else
      @message.tags << @tag unless @message.tags.include?(@tag)
    end

    dispatch_tag_notification(@tag)
    respond_to(&:turbo_stream)
  end

  def destroy
    @tag = @message.tags.find(params[:id])

    if @tag.external?
      begin
        label_assignment_service.new.remove(message: @message, tag: @tag)
      rescue Zoho::LabelAssignmentService::Error, Google::LabelAssignmentService::Error => e
        Rails.logger.error("[EmailMessageTags] Remove failed: #{e.message}")
      end
    else
      @message.tags.delete(@tag)
    end

    respond_to(&:turbo_stream)
  end

  private

  def label_assignment_service
    @message.email_account.google? ? Google::LabelAssignmentService : Zoho::LabelAssignmentService
  end

  def set_message
    @message = EmailMessage.where(email_account: Current.user.readable_email_accounts)
                           .find(params[:email_message_id])
  end

  def dispatch_tag_notification(tag)
    Current.workspace.users.find_each do |user|
      next if user == current_user

      user.notifications.create!(
        title: "Email tagged \"#{tag.name}\"",
        body: @message.subject.to_s.truncate(140),
        link_url: email_message_path(@message)
      )
    end
  end
end
