import HotwireNative
import UIKit

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Bundled fallback first, then the live server copy.
        Hotwire.loadPathConfiguration(from: [
            .file(Bundle.main.url(forResource: Config.bundledPathConfigurationName, withExtension: "json")!),
            .server(Config.remotePathConfigurationURL)
        ])

        // Route OAuth provider URLs to a real system browser *before* the default
        // SafariViewController handler can claim them. Order matters: handlers
        // are tried top-to-bottom and the first match wins.
        Hotwire.registerRouteDecisionHandlers([
            AppNavigationRouteDecisionHandler(),
            OAuthRouteDecisionHandler(),
            SafariViewControllerRouteDecisionHandler(),
            SystemNavigationRouteDecisionHandler()
        ])

        return true
    }

    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        UISceneConfiguration(name: "Default Configuration", sessionRole: connectingSceneSession.role)
    }
}
