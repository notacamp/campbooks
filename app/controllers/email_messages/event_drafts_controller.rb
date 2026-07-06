# frozen_string_literal: true

# Handles the inline event-draft block on the email show page.
# GET  :event_draft — lazy turbo-frame content: runs the heuristic extractor and
#   renders EventDraftBlock in :draft state, or an empty frame when preconditions
#   are not met (no writable calendar, no time proposal, self-sent email).
# POST :event_draft — "Add to calendar": calls Tools::CreateCalendarEvent (the
#   same path used by EmailActions and the Cmd+K palette) and responds with a
#   turbo_stream that replaces the frame with the :confirmed state on success,
#   or the :error state on failure.
class EmailMessages::EventDraftsController < ApplicationController
  before_action :require_authentication
  before_action :set_message

  def show
    # Cheap preconditions: skip the extractor when we know we won't show.
    unless show_block?
      render_empty_frame
      return
    end

    extractor = Ai::EventExtractor.new(@message)
    unless extractor.has_time_proposal?
      render_empty_frame
      return
    end

    @draft   = extractor.extract
    @edit_url = new_calendar_event_path(
      start: @draft.start_at.iso8601,
      end:   @draft.end_at.iso8601
    )
    @add_url = event_draft_email_message_path(@message)

    render layout: false
  end

  def create
    result = EmailActions.run("create_calendar_event", email_message: @message, user: Current.user)

    if result[:success]
      event = result.dig(:result, :event_id) && CalendarEvent.find_by(id: result.dig(:result, :event_id))
      block_html = render_to_string(
        Campbooks::EventDraftBlock.new(state: :confirmed, event: event),
        layout: false
      )
    else
      block_html = render_to_string(
        Campbooks::EventDraftBlock.new(
          state: :error,
          error_message: result[:message].presence || t(".error_generic"),
          add_url: event_draft_email_message_path(@message)
        ),
        layout: false
      )
    end

    render turbo_stream: turbo_stream.replace("event_draft_#{@message.id}", html: block_html)
  end

  private

  def set_message
    @message = EmailMessage.accessible_to(Current.user).find(params[:id])
  rescue ActiveRecord::RecordNotFound
    head :not_found
  end

  # Preconditions for showing the draft block (cheap, no AI call):
  #   1. User has at least one writable calendar they can create events on.
  #   2. The email was not sent by the inbox owner (outbound mail has no
  #      scheduling proposals to detect in the body).
  def show_block?
    return false if @message.sent?
    Calendar.where(
      calendar_account: Current.user.writable_calendar_accounts,
      is_writable: true,
      syncing: true
    ).exists?
  end

  def render_empty_frame
    render html: helpers.turbo_frame_tag("event_draft_#{@message.id}").html_safe, layout: false
  end
end
