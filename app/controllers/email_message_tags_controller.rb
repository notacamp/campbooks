class EmailMessageTagsController < ApplicationController
  before_action :require_authentication
  before_action :set_message

  def create
    @tag = Current.workspace.tags.find(params[:tag_id])
    @message.tags << @tag unless @message.tags.include?(@tag)
    dispatch_tag_notification(@tag)
    respond_to(&:turbo_stream)
  end

  def destroy
    @tag = @message.tags.find(params[:id])
    @message.tags.delete(@tag)
    respond_to(&:turbo_stream)
  end

  private

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
