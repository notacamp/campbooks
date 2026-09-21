module ApplicationCable
  class Connection < ActionCable::Connection::Base
    identified_by :current_user

    def connect
      set_current_user || reject_unauthorized_connection
    end

    private
      def set_current_user
        if user = (user_from_cookie || user_from_bearer)
          self.current_user = user
        end
      end

      # Same-origin web (Hotwire + the SPA behind Caddy) still authenticates the
      # cable connection with the signed session cookie.
      def user_from_cookie
        Session.find_by(id: cookies.signed[:session_id])&.user
      end

      # Cross-origin SPA / Capacitor: the connection carries no cookie, so it
      # passes the same /api/app Session bearer — as a query param on the cable
      # URL, since ActionCable's browser client can't set an Authorization header.
      # See api-migration/realtime.md.
      def user_from_bearer
        token = request.params[:token].presence || request.params[:bearer].presence
        Session.authenticate_api_token(token)&.user
      end
  end
end
