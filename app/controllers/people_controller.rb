# frozen_string_literal: true

# The People place (Rethink Stage 2): the workspace's persons and organizations
# ordered by who needs you, and a person opened as their email threads newest-first
# (subject headings, folded older messages, inline reply through the Compose Dock).
# Left pane = the counterpart list (People | Streams segmented control + search +
# Need-you / Recent sections); right pane = the selected person, loaded into the
# "people_detail" turbo frame (mirrors the email_detail pattern).
#
# Gated on Features.bold_layout? (PeopleLayout). Nothing here changes the send
# path or the mail model — it's a new reading of email threads + people + orgs.
class PeopleController < ApplicationController
  include PeopleLayout

  THREADS_PER_PAGE = 8

  def index
    build_people_list

    respond_to do |format|
      format.html do
        # Auto-open the top row on the full-page HTML response when no row is already
        # selected and this is not a Turbo Frame request (phones keep the list via the
        # start-on-list-value attribute).
        auto_open_top_row unless @selected_id.present? || turbo_frame_request?
        render :index
      end
      format.turbo_stream # the Recent list's lazy infinite-scroll sentinel
    end
  rescue Pagy::OverflowError
    respond_to do |format|
      format.turbo_stream { @recent = []; @recent_pagy = nil; render :index }
      format.html { redirect_to people_path(q: @query.presence) }
    end
  end

  def show
    @person = Current.workspace.people.find(params[:id])
    build_conversation

    respond_to do |format|
      format.turbo_stream # older threads: append next page + update sentinel
      format.html do
        if turbo_frame_request_id == "people_detail"
          render :show_detail, layout: false
        else
          build_people_list
          @selected_id = @person.id
          render :index
        end
      end
    end
  rescue Pagy::OverflowError
    respond_to do |format|
      format.turbo_stream { @conversation_threads = []; @conversation_pagy = nil; render :show }
      format.html { redirect_to person_page_path(@person) }
    end
  rescue ActiveRecord::RecordNotFound
    # A person from another workspace (or a stale id) is invisible, not forbidden.
    head :not_found
  end

  private

  # Each person's threads ordered by their latest accessible message (newest-first),
  # paginated. Renders as ThreadBlock components: subject heading, older messages
  # folded, the newest open, and an inline reply row under the newest thread.
  def build_conversation
    contact_ids = @person.contacts.ids
    thread_ids = conversation_thread_ids(contact_ids)

    # Order threads by the newest accessible message in each.
    latest_per_thread = EmailMessage.where(email_thread_id: thread_ids)
                                    .accessible_to(current_user)
                                    .group(:email_thread_id)
                                    .maximum(:received_at)
    ordered_ids = latest_per_thread.sort_by { |_id, at| at || Time.at(0) }.map(&:first).reverse

    @conversation_pagy, page_ids = pagy_array(ordered_ids, limit: THREADS_PER_PAGE)

    threads_by_id = EmailThread.where(id: page_ids).index_by(&:id)
    messages_by_thread = EmailMessage.where(email_thread_id: page_ids)
                                     .accessible_to(current_user)
                                     .includes(:contact, :email_account, files_attachments: :blob)
                                     .order(:received_at, :id)
                                     .group_by(&:email_thread_id)

    @conversation_threads = page_ids.filter_map do |tid|
      thread = threads_by_id[tid]
      msgs = messages_by_thread[tid]
      next unless thread && msgs&.any?

      People::ConversationThread.new(thread: thread, messages: msgs)
    end

    @newest_thread = @conversation_pagy.page == 1 ? @conversation_threads.first : nil

    reply_msg = @newest_thread&.reply_target
    reply_msg ||= contact_ids.any? ? EmailMessage.where(contact_id: contact_ids)
                                                  .accessible_to(current_user)
                                                  .order(received_at: :desc).first : nil

    @reply_target = reply_msg
    @can_send = @reply_target ? @reply_target.email_account.sendable_by?(current_user) : false
    @scout_draft = @newest_thread ? Emails::ScoutDraft.for(@newest_thread&.newest) : nil

    @standing = PeopleStanding.for_user(current_user).find_by(counterpart: @person)&.standing ||
                People::Standing.for_person(@person, user: current_user)
    @primary_contact = @person.contacts.max_by { |c| c.email_count.to_i }
    @email_total = @person.total_email_count
    @first_seen = contact_ids.any? ? EmailMessage.where(contact_id: contact_ids).minimum(:received_at) : nil
    @sender_tags = @person.contacts.flat_map(&:sender_tags).uniq.first(2)
  end

  def conversation_thread_ids(contact_ids)
    return [] if contact_ids.empty?

    EmailMessage.where(contact_id: contact_ids).where.not(email_thread_id: nil)
                .distinct.pluck(:email_thread_id)
  end

  # Pre-render the detail pane for the first row in the list so the HTML response
  # opens with content rather than a blank right pane. Sets @auto_opened = true so
  # the view passes start-on-list-value="true" (phones keep the list, not the detail).
  def auto_open_top_row
    candidates = (@lanes || []).flat_map { |lane| lane[:counterparts] } + Array(@recent)
    top = candidates.find(&:person?)
    return unless top

    @person = Current.workspace.people.find_by(id: top.id)
    return unless @person

    build_conversation
    @selected_id = top.id
    @auto_opened = true
  rescue ActiveRecord::RecordNotFound
    @person = nil
  end
end
