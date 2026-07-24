# frozen_string_literal: true

# Skim Mode: the user's inbox grouped into category "rings" of cluster cards and
# reviewed one stack at a time, Instagram-stories style. Categorisation +
# clustering are free (rules only). The swipe decisions only *flag* clusters; the
# archive is applied once, explicitly, via #decide.
class SkimController < ApplicationController
  before_action :require_authentication

  def show
    # Apply AI cluster summaries only when the viewer actually opens (not on every
    # inbox tray load), so we never spend tokens summarising mail no one skims.
    @rings = Emails::SkimSummaries.new(Current.user).apply!(current_rings)
    @start_theme = params[:start].presence
    @standalone = !turbo_frame_request?

    # When loaded into the inbox overlay (a turbo-frame request) skip the app
    # chrome so only the skim_content frame is returned (no duplicate frame ids).
    render layout: false if turbo_frame_request?
  end

  # The inbox ring tray (lazily loaded into a turbo-frame so it never blocks the
  # inbox render). Same rings as the viewer, so a ring deep-links into its story.
  # `doc_count` (passed by home, absent on the inbox) folds the document-review
  # queue into the "Skim all" badge + seamless chain; mail-only when 0.
  def tray
    @rings = current_rings
    @doc_count = params[:doc_count].to_i
    render layout: false
  end

  # Archives one cluster's emails, applied immediately as the user acts on a card.
  # Security-scoped inside Emails::SkimArchive — only the current user's own mail
  # can be touched. The viewer offers an Undo (see #undo) right after.
  def decide
    archived = Emails::SkimArchive.new(Current.user, params[:email_ids]).call
    record_decision("archive")
    refresh_tray
    render json: { archived: archived }
  end

  # Reverses a just-applied archive (the Undo on the archive toast): moves the
  # emails back to their inbox. Same security scoping as #decide.
  def undo
    restored = Emails::SkimRestore.new(Current.user, params[:email_ids]).call
    refresh_tray
    render json: { restored: restored }
  end

  # Marks a cluster as addressed ("Keep" — kept in the inbox but handled), so the
  # feed never re-surfaces it. Security-scoped inside Emails::SkimDismiss. No live
  # broadcast: Keep fires on every advance, so the tray is refreshed once when the
  # overlay closes (skim-overlay#close) rather than on every card.
  def keep
    kept = Emails::SkimDismiss.new(Current.user, params[:email_ids]).call
    record_decision("keep")
    render json: { kept: kept }
  end

  # Promotes a cluster into the Priority lane (pins it above time ordering).
  # Security-scoped inside Emails::SkimPromote.
  def promote
    promoted = Emails::SkimPromote.new(Current.user, params[:email_ids]).call
    record_decision("promote")
    refresh_tray
    render json: { promoted: promoted }
  end

  # Removes emails from the Priority lane. Security-scoped inside SkimUnpromote.
  def unpromote
    unpromoted = Emails::SkimUnpromote.new(Current.user, params[:email_ids]).call
    refresh_tray
    render json: { unpromoted: unpromoted }
  end

  # Retires the follow-up on a card's thread(s) so it stops surfacing in Skim AND
  # the feed (sets follow_up_dismissed_at). Scoped to the user's readable accounts.
  def dismiss_follow_up
    thread_ids = EmailMessage.where(email_account: Current.user.readable_email_accounts, id: params[:email_ids])
                             .distinct.pluck(:email_thread_id).compact
    dismissed = EmailThread.where(id: thread_ids).update_all(follow_up_dismissed_at: Time.current)
    refresh_tray
    render json: { dismissed: dismissed }
  end

  # Runs a sender-scoped registry action (star / block / allow / …) from a Skim
  # card and refreshes the tray. The action mutates the sender's Contact, so any
  # one of the card's emails resolves it (a pending/starred card is one sender).
  # Restricted to sender tools surfaced in Skim; the registry re-checks access.
  def sender_action
    tool = params[:tool].to_s
    return render(json: { error: "unknown action" }, status: :unprocessable_entity) unless skim_sender_tool?(tool)

    email = EmailMessage.where(email_account: Current.user.readable_email_accounts)
                        .where(id: params[:email_ids])
                        .order(received_at: :desc).first
    return render(json: { error: "no email" }, status: :not_found) unless email

    result = EmailActions.run(tool, email_message: email, user: Current.user)
    refresh_tray
    render json: { success: result[:success], message: result[:message] }
  end

  # Renders one email as a card (stacked over the stack) into the shared
  # skim_email_card frame. Scoped to the current user's own mail. @can_send gates
  # the inline Reply affordance to accounts the user may send from.
  def email
    @email = EmailMessage.where(email_account: Current.user.readable_email_accounts).find(params[:id])
    @can_send = @email.email_account.sendable_by?(Current.user)
    render layout: false
  end

  # Just the sanitized email body, loaded lazily into a single-email Skim card's
  # turbo-frame so the whole content shows inline (no extra click). Scoped to the
  # current user's own mail. `fallback=summary` (set by single-email cards) shows
  # the email's summary when the stored body is empty, so the one email a card
  # holds always reads as content rather than a "No preview" placeholder.
  def email_content
    @email = EmailMessage.where(email_account: Current.user.readable_email_accounts).find(params[:id])
    @summary_fallback = params[:fallback] == "summary"
    render layout: false
  end

  # Sends an inline reply from the stacked email card. Permission-gated to
  # sendable accounts; mirrors EmailToolsController#send_reply's provider calls
  # (save a draft, then send it) without the agent-thread bookkeeping.
  def reply
    email = EmailMessage.where(email_account: Current.user.readable_email_accounts).find(params[:id])

    unless email.email_account.sendable_by?(Current.user)
      return render json: { error: t(".no_send_permission") }, status: :forbidden
    end

    body = params[:body].to_s
    return render json: { error: t(".empty_body") }, status: :unprocessable_entity if body.strip.empty?

    client = email.email_account.mail_client
    draft = client.save_draft(subject: "Re: #{email.subject}", body: body,
                              to_address: Emails::ComposePrefill.reply_to_address(email))
    draft_id = draft && (draft["messageId"] || draft["id"])
    return render json: { error: t(".send_failed") }, status: :unprocessable_entity unless draft_id

    client.send_draft(draft_id)
    # Mark the thread as awaiting their reply so it drops from the deck right away;
    # the follow-up verdict is computed when the sent copy syncs back (EmailProcessJob).
    email.email_thread&.update_column(:last_outbound_at, Time.current)
    render json: { sent: true }
  rescue ActiveRecord::RecordNotFound
    raise
  rescue => e
    Rails.logger.error("[Skim#reply] #{e.class}: #{e.message}")
    render json: { error: t(".send_failed") }, status: :unprocessable_entity
  end

  private

  # The user's Skim rings, built from the shared time-windowed scope (the same
  # scope the real-time broadcaster uses, so the inbox tray and viewer agree).
  def current_rings
    Emails::SkimDeck.for(
      Current.user,
      now: Time.current,
      whitelist_mode: Current.workspace&.whitelist_mode?,
      memory: Emails::SkimActionMemory.new(Current.user)
    )
  end

  # Log the user's choice on this cluster so Skim learns their habit and can
  # pre-suggest the same action (as Scout) on the next similar card. Best-effort:
  # the recorder swallows its own errors, so triage never fails on a logging hiccup.
  def record_decision(action)
    Emails::SkimDecisionRecorder.record(Current.user, params[:email_ids], action: action)
  end

  # A sender-scoped registry tool that's allowed from the Skim surface.
  def skim_sender_tool?(tool)
    defn = EmailActions.definition(tool)
    defn&.target == :sender && defn.surfaces.include?(:skim)
  end

  # Push the refreshed tray to the user's stream so every open session updates
  # live (the inbox subscribes via turbo_stream_from "skim_#{user.id}").
  def refresh_tray
    Emails::SkimTrayBroadcaster.refresh(Current.user)
  end
end
