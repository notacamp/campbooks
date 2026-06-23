import AuthenticationServices
import HotwireNative
import UIKit

/// Opens OAuth provider URLs (Google / Microsoft / Zoho) in a real system
/// browser via `ASWebAuthenticationSession` instead of the embedded web view —
/// providers reject embedded webviews ("disallowed_useragent").
///
/// The server bakes a signed, native-aware `state` into the authorize URL (see
/// the Rails `Oauth::State` + `OauthNativeHandoff`). When the dance finishes the
/// server redirects to `campbooks://oauth?...`, which the auth session
/// intercepts and hands back here:
///   • flow=signin  → load `/session/native?token=…` in the web view so the
///     session cookie lands in the web view's own cookie store.
///   • flow=connect → the account is already linked server-side; reload the
///     accounts screen.
final class OAuthRouteDecisionHandler: NSObject, RouteDecisionHandler {
    let name = "oauth-provider"

    /// Held so the in-flight auth session isn't deallocated before it completes.
    private var authSession: ASWebAuthenticationSession?

    func matches(location: URL, configuration: Navigator.Configuration) -> Bool {
        guard let host = location.host else { return false }
        return Config.oauthProviderHosts.contains(host)
    }

    func handle(
        location: URL,
        configuration: Navigator.Configuration,
        navigator: Navigator
    ) -> Router.Decision {
        // A single tap can propose this navigation more than once (the WebView
        // visit + Hotwire Native's URLSession redirect re-resolution), which
        // would otherwise open a second browser. Only ever run one at a time.
        guard authSession == nil else {
            NSLog("[OAUTH-iOS] duplicate handle() for %@ — suppressed", location.absoluteString)
            return .cancel
        }
        NSLog("[OAUTH-iOS] opening auth session for %@", location.absoluteString)

        let session = ASWebAuthenticationSession(
            url: location,
            callbackURLScheme: Config.oauthCallbackScheme
        ) { [weak self, weak navigator] callbackURL, error in
            NSLog("[OAUTH-iOS] auth session finished — callback=%@ error=%@",
                  callbackURL?.absoluteString ?? "nil", String(describing: error))
            self?.authSession = nil
            guard let callbackURL, let navigator else { return }
            Self.handleCallback(callbackURL, navigator: navigator)
        }

        session.presentationContextProvider = self
        // Share Safari cookies so an already-signed-in provider session is reused.
        session.prefersEphemeralWebBrowserSession = false
        authSession = session
        session.start()

        // We've taken over; never navigate the provider URL in-app.
        return .cancel
    }

    private static func handleCallback(_ url: URL, navigator: Navigator) {
        NSLog("[OAUTH-iOS] handleCallback %@", url.absoluteString)
        let query = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems ?? []
        let params = Dictionary(query.compactMap { item in item.value.map { (item.name, $0) } },
                                uniquingKeysWith: { first, _ in first })

        switch params["flow"] {
        case "signin":
            guard let token = params["token"] else { return }
            var components = URLComponents(
                url: Config.rootURL.appendingPathComponent("session/native"),
                resolvingAgainstBaseURL: false
            )!
            components.queryItems = [URLQueryItem(name: "token", value: token)]
            navigator.route(components.url!)

        case "connect":
            // Account linked server-side already; reload the accounts screen.
            var components = URLComponents(
                url: Config.rootURL.appendingPathComponent("email_messages"),
                resolvingAgainstBaseURL: false
            )!
            components.queryItems = [URLQueryItem(name: "inbox_settings", value: "accounts")]
            navigator.route(components.url!)

        default:
            break // status=error or unknown — leave the user where they are.
        }
    }
}

extension OAuthRouteDecisionHandler: ASWebAuthenticationPresentationContextProviding {
    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows)
            .first(where: \.isKeyWindow) ?? ASPresentationAnchor()
    }
}
