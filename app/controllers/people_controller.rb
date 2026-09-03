# frozen_string_literal: true

require "pagy/extras/countless"

# The People place (Rethink Stage 2): the workspace's persons and organizations
# ordered by who needs you, and a person opened as ONE conversation across all
# their threads — Scout's summary pinned on top, the reply box docked at the
# bottom. Left pane = the counterpart list (People | Streams segmented control +
# search + Need-you / Recent sections); right pane = the selected person, loaded
# into the "people_detail" turbo frame (mirrors the email_detail pattern).
#
# Gated on Features.bold_layout? (PeopleLayout). Nothing here changes the send
# path or the mail model — it's a new reading of email threads + people + orgs.
class PeopleController < ApplicationController
  include PeopleLayout

  CONVERSATION_PER_PAGE = 30

  def index
    build_people_list

    respond_to do |format|
      format.html
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
      format.turbo_stream # "Earlier" — prepend the previous (older) page
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
  rescue ActiveRecord::RecordNotFound
    # A person from another workspace (or a stale id) is invisible, not forbidden.
    head :not_found
  end

  private

  # The whole relationship in one scroll: every message across the person's
  # threads (their inbound + your outbound in those threads), newest-first for
  # pagination, then reversed so the view reads oldest→newest with the newest at
  # the bottom next to the reply box. "Earlier" loads the older page on top.
  def build_conversation
    contact_ids = @person.contacts.ids
    thread_ids = conversation_thread_ids(contact_ids)

    base = EmailMessage.where(email_thread_id: thread_ids)
                       .accessible_to(current_user)
                       .includes(:email_account, :contact, :email_thread, files_attachments: :blob)
                       .order(received_at: :desc)
    @conversation_pagy, page = pagy_countless(base, limit: CONVERSATION_PER_PAGE)
    @conversation_messages = page.to_a.reverse
    @newest_message = @conversation_messages.last

    @standing = People::Standing.for_person(@person, user: current_user)
    @primary_contact = @person.contacts.max_by { |c| c.email_count.to_i }
    @email_total = @person.total_email_count
    @first_seen = contact_ids.any? ? EmailMessage.where(contact_id: contact_ids).minimum(:received_at) : nil
    @sender_tags = @person.contacts.flat_map(&:sender_tags).uniq.first(2)

    build_reply_dock(contact_ids)
  end

  def conversation_thread_ids(contact_ids)
    return [] if contact_ids.empty?

    EmailMessage.where(contact_id: contact_ids).where.not(email_thread_id: nil)
                .distinct.pluck(:email_thread_id)
  end

  # The reply box docks against the person's most recent inbound message and
  # opens the existing Compose Dock in reply mode — the send path is unchanged.
  # When Scout has a reply ready (the message carries an action prompt), its text
  # previews in the dock under the "Scout's draft · edit freely" chip; the user
  # still reviews and sends from the Dock.
  def build_reply_dock(contact_ids)
    @reply_target = contact_ids.any? ? EmailMessage.where(contact_id: contact_ids)
                                                   .accessible_to(current_user)
                                                   .order(received_at: :desc).first : nil
    @can_send = @reply_target ? @reply_target.email_account.sendable_by?(current_user) : false
    @scout_draft = @reply_target&.ai_action_prompt.to_s.strip.presence
  end
end
