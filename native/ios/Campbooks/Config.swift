import Foundation

/// Single source of app-wide configuration. Most things you'd tweak per
/// environment live here.
enum Config {
    /// Where the app loads from. Release builds (App Store / TestFlight) always
    /// point at production; Debug builds use `http://localhost:3000` for the iOS
    /// Simulator against a local Rails server, which is allowed in Info.plist's
    /// App Transport Security exception (see the commented block there).
    #if DEBUG
    static let rootURL = URL(string: "http://localhost:3000")!
    #else
    static let rootURL = URL(string: "https://app.campbooks.not-a-camp.com")!
    #endif

    /// Bundled path-configuration fallback (used at first launch / offline). The
    /// live copy is fetched from the server so navigation rules can change
    /// without shipping an app update. Keep this file in sync with the Rails
    /// `config/hotwire/path_configuration.json`.
    static let bundledPathConfigurationName = "path-configuration"

    static var remotePathConfigurationURL: URL {
        rootURL.appendingPathComponent("configurations/ios_v1.json")
    }

    // MARK: - OAuth handoff (see OAuthRouteDecisionHandler + the Rails
    // OauthNativeHandoff concern).

    /// Custom URL scheme the OAuth flow redirects back to. Must match the
    /// `campbooks` scheme registered in Info.plist and the scheme the Rails
    /// callbacks redirect to (`campbooks://oauth?...`).
    static let oauthCallbackScheme = "campbooks"

    /// OAuth provider hosts that must open in a real system browser
    /// (ASWebAuthenticationSession), never the embedded web view — Google &
    /// others reject embedded webviews ("disallowed_useragent").
    static let oauthProviderHosts: Set<String> = [
        "accounts.google.com",
        "login.microsoftonline.com",
        "accounts.zoho.eu",
        "accounts.zoho.com"
    ]
}
