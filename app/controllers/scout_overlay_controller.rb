# frozen_string_literal: true

# Serves the body of the global Scout overlay (Campbooks::ScoutOverlay). The
# overlay's <dialog> shell lives in the layout; its inner turbo-frame lazily
# loads this action the first time the overlay opens, so no request is made on a
# normal page load.
#
# Two modes:
#   idle          — Scout suggestions + the last 6 Scout threads (Recent); the
#                   command catalog + live search are rendered client-side by
#                   scout_overlay_controller.js.
#   conversation  — ?thread_id= renders that thread's last 20 messages, wired to
#                   the SAME turbo-stream target ids the reply job broadcasts to
#                   (agent_messages_list / agent_typing), so asking a question
#                   streams the reply in place.
class ScoutOverlayController < ApplicationController
  before_action :require_authentication

  layout false

  def show
    @ai_available = ai_provider_available?(:text)
    @recent = recent_threads
    @recent_count = @recent.size

    if params[:current].present?
      # The user asked from browse mode: continue (or start) the global thread.
      # default_for returns the latest global thread with messages, else creates one.
      @thread = AgentThread.default_for(current_user)
      @messages = @thread.agent_messages.chronological.last(20)
      render :conversation
    elsif params[:thread_id].present?
      # find on the user's own visible threads: another user's (or a setup) thread
      # 404s (RecordNotFound) rather than leaking its existence.
      @thread = current_user.agent_threads.scout_visible.find(params[:thread_id])
      @messages = @thread.agent_messages.chronological.last(20)
      render :conversation
    else
      @suggestions = Scout::Briefing.for(current_user)[:suggestions]
      render :idle
    end
  rescue ActiveRecord::RecordNotFound
    head :not_found
  end

  private

  # The last 6 Scout threads that carry messages, most-recent first. `.to_a` is
  # deliberate: `with_messages` groups by thread id, so a bare `.count` would
  # return a per-group hash rather than a number.
  def recent_threads
    current_user.agent_threads.scout_visible.with_messages.recent.limit(6).to_a
  end
end
