# frozen_string_literal: true

require "set"

module Emails
  # The single entry point both SkimController and SkimTrayBroadcaster use to build
  # Skim's rings, so the inbox tray and the viewer can never drift (until now the
  # broadcaster silently omitted `memory:`).
  #
  # It sits between Emails::SkimScope (the raw time-windowed relation) and
  # Emails::SkimBuilder (the pure grouper), doing the DB work the builder must not:
  #
  #   1. HIDE answered conversations — threads where the owner already had the last
  #      word (they replied, the other party hasn't come back) and no follow-up is
  #      due. This is the core "stop re-showing mail I replied to" fix.
  #   2. SURFACE follow-ups — threads the owner is waiting on a reply for whose
  #      nudge time has come (Emails::AwaitingReply#due; pure data — the AI only
  #      refines the timing, so this fires even with no AI provider). These may sit
  #      OUTSIDE Skim's 14-day window, so we pull each representative message in.
  #
  # The builder then routes the follow-up threads into a leading "Follow-ups" ring
  # via the `follow_up_thread_ids` set we pass it (it stays pure — no DB).
  class SkimDeck
    def self.for(user, now: Time.current, whitelist_mode: false, memory: nil)
      new(user, now: now, whitelist_mode: whitelist_mode, memory: memory).rings
    end

    def initialize(user, now: Time.current, whitelist_mode: false, memory: nil)
      @user = user
      @now = now
      @whitelist_mode = whitelist_mode
      @memory = memory
    end

    def rings
      accounts = @user.readable_email_accounts.to_a
      return [] if accounts.empty?

      emails = SkimScope.for(@user).to_a
      state  = thread_state(emails)

      due           = Emails::AwaitingReply.new(@user, now: @now).due
      follow_up_ids = due.map(&:id).to_set
      follow_up_meta = due.each_with_object({}) do |t, h|
        h[t.id] = { reason: t.follow_up_reason, at: t.follow_up_at }
      end

      kept = emails.reject { |email| hide?(email, state, follow_up_ids) }
      merged = (kept + follow_up_representatives(follow_up_ids, accounts)).uniq(&:id)

      SkimBuilder.new(
        merged,
        now: @now,
        whitelist_mode: @whitelist_mode,
        memory: @memory,
        follow_up_thread_ids: follow_up_ids,
        follow_up_meta: follow_up_meta
      ).rings
    end

    private

    # Hide a conversation the owner already answered — unless a follow-up is due
    # (then it's kept and routed to the Follow-ups ring). Threadless mail and
    # still-their-turn threads always stay.
    def hide?(email, state, follow_up_ids)
      tid = email.email_thread_id
      return false if tid.nil?
      return false if follow_up_ids.include?(tid)

      state[tid]&.holds_last_word? || false
    end

    # Minimal thread records (just the reply-state columns) for the in-window mail,
    # keyed by id. One query, no N+1.
    def thread_state(emails)
      ids = emails.filter_map(&:email_thread_id).uniq
      return {} if ids.empty?

      EmailThread
        .where(id: ids)
        .select(:id, :last_outbound_at, :last_inbound_at, :follow_up_expected,
                :follow_up_at, :follow_up_reason, :follow_up_dismissed_at)
        .index_by(&:id)
    end

    # The newest non-skimmed INBOX message per follow-up-due thread — the card we
    # show for "you're waiting on a reply". Covers threads outside the 14-day window
    # that SkimScope didn't return. Admission (blocked/pending) is left to the
    # builder, exactly as SkimScope relies on it. A thread with no inbox message
    # (e.g. a cold outbound, or the original was archived) yields nothing.
    def follow_up_representatives(follow_up_ids, accounts)
      return [] if follow_up_ids.empty?

      scope = EmailMessage.where(email_account: accounts, skimmed_at: nil,
                                 email_thread_id: follow_up_ids.to_a)
      scope = Emails::InboxFolders.constrain(scope, accounts)
      scope.includes(:contact, :tags)
           .select(*SkimScope::SELECT)
           .order(received_at: :desc)
           .group_by(&:email_thread_id)
           .values
           .map(&:first)
    end
  end
end
