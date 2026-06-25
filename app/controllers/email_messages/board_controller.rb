# frozen_string_literal: true

# Status board (kanban) view of the inbox. Four columns over the user's readable
# mailboxes — Inbox / Snoozed / Awaiting reply / Done — rendered into the
# `inbox_board` turbo-frame. Dragging a card between columns maps to a reversible
# EmailActions tool on the :board surface. "Awaiting reply" is AI-computed and
# read-only: cards can be dragged OUT of it, never INTO it.
class EmailMessages::BoardController < ApplicationController
  before_action :require_email_board_enabled
  before_action :require_authentication

  COLUMN_LIMIT = 50
  AGED_DAYS = 3

  def index
    @columns = build_columns
    render layout: false
  end

  # Drag-and-drop target. JS posts { thread_id, from, to } (+ optional
  # snoozed_until). Each (from → to) maps to a reversible tool; drops into the
  # read-only Awaiting column, and no-op moves, are rejected.
  def move
    thread = readable_threads.find_by(id: params[:thread_id])
    message = thread&.latest_message
    return render(json: { ok: false, error: t(".not_found") }, status: :not_found) unless message

    tool, args = transition_for(params[:from].to_s, params[:to].to_s)
    return render(json: { ok: false, error: t(".invalid_move") }, status: :unprocessable_entity) unless tool

    result = EmailActions.run(tool, email_message: message, args: args, user: Current.user)
    if result[:success]
      render json: { ok: true, message: result[:message] }
    else
      render json: { ok: false, error: result[:message] }, status: :unprocessable_entity
    end
  end

  private

  # --- columns --------------------------------------------------------------

  def build_columns
    accounts     = Current.user.readable_email_accounts.to_a
    inbox_ids    = Emails::InboxFolders.ids_for(accounts)
    archive_ids  = Emails::ArchiveFolders.ids_for(accounts)
    awaiting_ids = awaiting_thread_ids(inbox_ids)

    [
      column(:inbox,    inbox_scope(inbox_ids, awaiting_ids), draggable: true),
      column(:snoozed,  snoozed_scope,                        draggable: true),
      column(:awaiting, awaiting_scope(awaiting_ids),         draggable: false),
      column(:done,     done_scope(archive_ids),              draggable: true)
    ]
  end

  # Load one extra row to detect overflow without a COUNT (the column scopes are
  # GROUP BY'd, where COUNT returns a hash).
  def column(key, scope, draggable:)
    rows = scope.limit(COLUMN_LIMIT + 1).to_a
    {
      key: key,
      threads: rows.first(COLUMN_LIMIT),
      has_more: rows.size > COLUMN_LIMIT,
      draggable: draggable
    }
  end

  def inbox_scope(inbox_ids, awaiting_ids)
    return EmailThread.none if inbox_ids.empty?

    scope = ordered_threads.where(email_messages: { provider_folder_id: inbox_ids })
                           .where("email_threads.snoozed_until IS NULL OR email_threads.snoozed_until <= ?", Time.current)
    scope = scope.where.not(id: awaiting_ids) if awaiting_ids.any?
    scope
  end

  def snoozed_scope
    EmailThread.snoozed
               .where(email_account_id: readable_account_ids)
               .includes(:email_account, email_messages: :email_account)
               .order(snoozed_until: :asc)
  end

  def awaiting_scope(awaiting_ids)
    return EmailThread.none if awaiting_ids.empty?

    ordered_threads.where(id: awaiting_ids)
  end

  def done_scope(archive_ids)
    return EmailThread.none if archive_ids.empty?

    ordered_threads.where(email_messages: { provider_folder_id: archive_ids })
  end

  # Threads expecting a reply: Scout flagged a draft_reply / high priority, aged
  # past AGED_DAYS, the owner hasn't responded yet, restricted to inbox folders so
  # it stays disjoint from Snoozed/Done. Mirrors Feed::Sources::ReplyReminder.
  def awaiting_thread_ids(inbox_ids)
    return [] if inbox_ids.empty?

    expects_reply = EmailMessage.where("ai_suggested_actions @> ?", [ { tool: "draft_reply" } ].to_json)
                                .or(EmailMessage.where(ai_priority: :high))

    EmailMessage.accessible_to(Current.user).with_ai_todos
                .where(provider_folder_id: inbox_ids, skimmed_at: nil)
                .where("received_at < ?", AGED_DAYS.days.ago)
                .merge(expects_reply)
                .includes(:email_thread, :email_account)
                .order(received_at: :desc)
                .limit(200)
                .reject { |m| replied?(m) }
                .filter_map(&:email_thread_id)
                .uniq
                .first(COLUMN_LIMIT)
  end

  # True once the mailbox owner sent a later message into the thread.
  def replied?(message)
    thread = message.email_thread
    return false unless thread

    addr = message.email_account&.email_address.to_s.downcase
    return false if addr.blank?

    thread.email_messages
          .where("received_at > ?", message.received_at)
          .where("LOWER(from_address) = ?", addr)
          .exists?
  end

  # --- drag transitions -----------------------------------------------------

  def transition_for(from, to)
    case to
    when "done"    then [ "archive", {} ]
    when "snoozed" then [ "snooze", { "snoozed_until" => snooze_until_param } ]
    when "inbox"
      case from
      when "done"    then [ "unarchive", {} ]
      when "snoozed" then [ "unsnooze", {} ]
      else [ nil, nil ] # already in the inbox (incl. Awaiting → Inbox): nothing to do
      end
    else [ nil, nil ] # unknown target, or a drop into the read-only Awaiting column
    end
  end

  def snooze_until_param
    params[:snoozed_until].presence || (Time.current + 1.day).change(hour: 8).iso8601
  end

  # --- scoping helpers ------------------------------------------------------

  def readable_account_ids
    @readable_account_ids ||= Current.user.readable_email_accounts.pluck(:id)
  end

  def readable_threads
    EmailThread.where(email_account_id: readable_account_ids)
  end

  def ordered_threads
    readable_threads
      .includes(:email_account, email_messages: :email_account)
      .joins(:email_messages)
      .group("email_threads.id")
      .order(Arel.sql("MAX(email_messages.received_at) DESC"))
  end
end
