import HotwireNative
import UIKit

class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?

    // Lazy so `self` is available as the navigator delegate.
    private lazy var navigator = Navigator(
        configuration: .init(name: "main", startLocation: Config.rootURL),
        delegate: self
    )

    func scene(
        _ scene: UIScene,
        willConnectTo session: UISceneSession,
        options connectionOptions: UIScene.ConnectionOptions
    ) {
        guard let windowScene = scene as? UIWindowScene else { return }

        let window = UIWindow(windowScene: windowScene)
        window.rootViewController = navigator.rootViewController
        self.window = window
        window.makeKeyAndVisible()

        navigator.start()
    }
}

extension SceneDelegate: NavigatorDelegate {
    // Route every screen through our contextual controller so the native nav bar
    // is managed per-screen (hidden on tab roots, shown on pushed detail screens).
    func handle(proposal: VisitProposal, from navigator: Navigator) -> ProposalResult {
        .acceptCustom(WebViewController(url: proposal.url))
    }
}
